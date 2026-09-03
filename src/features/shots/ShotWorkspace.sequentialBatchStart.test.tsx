// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProductionBatchDetail, ProductionBatchSummary, ProductionQueueOverview } from "../../types/productionQueue";
import type { ProductionBatchReviewProductivity } from "../../services/tauriClient";
import type { TaskView } from "../../types/task";
import { ShotWorkspace } from "./ShotWorkspace";

const ids = ["batch-a", "batch-b", "batch-c", "batch-d"] as const;
type BatchId = (typeof ids)[number];

const mocks = vi.hoisted(() => ({
  listShots: vi.fn(),
  getProjectWorkflowConfig: vi.fn(),
  listRecentAssets: vi.fn(),
  listPromptLibrary: vi.fn(),
  listReferenceAnchors: vi.fn(),
  listProductionStructure: vi.fn(),
  getProductionBatchRunbook: vi.fn(),
  listBatchWorkflowPresets: vi.fn(),
  listProductionQueues: vi.fn(),
  getProductionAdmissionStatus: vi.fn(),
  getProductionQueue: vi.fn(),
  getProductionQueueOverview: vi.fn(),
  getProductionBatchReviewProductivity: vi.fn(),
  getAssetMediaUrl: vi.fn(),
  startProductionQueue: vi.fn(),
  pauseProductionQueue: vi.fn(),
  requeueProductionQueueItem: vi.fn(),
  revealProductionReviewAsset: vi.fn(),
  openProductionReviewOutputFolder: vi.fn(),
  subscribeTaskUpdates: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return {
    ...actual,
    listShots: mocks.listShots,
    getProjectWorkflowConfig: mocks.getProjectWorkflowConfig,
    listRecentAssets: mocks.listRecentAssets,
    listPromptLibrary: mocks.listPromptLibrary,
    listReferenceAnchors: mocks.listReferenceAnchors,
    listProductionStructure: mocks.listProductionStructure,
    getProductionBatchRunbook: mocks.getProductionBatchRunbook,
    listBatchWorkflowPresets: mocks.listBatchWorkflowPresets,
    listProductionQueues: mocks.listProductionQueues,
    getProductionAdmissionStatus: mocks.getProductionAdmissionStatus,
    getProductionQueue: mocks.getProductionQueue,
    getProductionQueueOverview: mocks.getProductionQueueOverview,
    getProductionBatchReviewProductivity: mocks.getProductionBatchReviewProductivity,
    getAssetMediaUrl: mocks.getAssetMediaUrl,
    startProductionQueue: mocks.startProductionQueue,
    pauseProductionQueue: mocks.pauseProductionQueue,
    requeueProductionQueueItem: mocks.requeueProductionQueueItem,
    revealProductionReviewAsset: mocks.revealProductionReviewAsset,
    openProductionReviewOutputFolder: mocks.openProductionReviewOutputFolder,
  };
});

vi.mock("../../services/taskEvents", () => ({ subscribeTaskUpdates: mocks.subscribeTaskUpdates }));
vi.mock("./ProjectStructureTree", () => ({ ProjectStructureTree: () => <div aria-label="项目结构" /> }));
vi.mock("./ShotListToolbar", () => ({ ShotListToolbar: () => <div aria-label="镜头搜索和筛选" /> }));
vi.mock("./ProjectProductionPipeline", () => ({ ProjectProductionPipeline: () => <div aria-label="项目生产流水线" /> }));
vi.mock("./ShotCreationWorkspace", () => ({ ShotCreationWorkspace: () => <div aria-label="镜头创建工作区" /> }));
vi.mock("./ShotBatchReviewBoard", () => ({ ShotBatchReviewBoard: () => <div aria-label="镜头批次审核" /> }));
vi.mock("./SceneProductionPanel", () => ({ SceneProductionPanel: () => <div aria-label="场景生产" /> }));
vi.mock("./EpisodeProductionPanel", () => ({ EpisodeProductionPanel: () => <div aria-label="集生产" /> }));
vi.mock("./SeriesProductionPanel", () => ({ SeriesProductionPanel: () => <div aria-label="系列生产" /> }));
vi.mock("./ScopeConsistencyWorkspace", () => ({ ScopeConsistencyWorkspace: () => <div aria-label="一致性工作区" /> }));
vi.mock("../production/ProductionPackageWorkspace", () => ({ ProductionPackageWorkspace: () => <div aria-label="生产包工作区" /> }));
vi.mock("../production/ProductionBatchRunbookPanel", () => ({ ProductionBatchRunbookPanel: () => <div aria-label="生产手册" /> }));
vi.mock("../production/MultiPackageProductionBoard", () => ({ MultiPackageProductionBoard: () => <div aria-label="多生产包看板" /> }));
vi.mock("../production/ProductionMonitor", () => ({ ProductionMonitor: () => <div data-testid="production-monitor" /> }));
vi.mock("../studio/ProductionAssetPreview", () => ({ ProductionAssetPreview: () => <div aria-label="资产预览" /> }));

let batchStatuses: Record<BatchId, ProductionBatchDetail["status"]>;
let batchCounts: Record<BatchId, { total: number; succeeded: number; failed: number; cancelled: number; skipped: number }>;
let activeBatchId: BatchId | undefined;
let taskUpdateListener: ((task: TaskView) => void) | undefined;
let startFailure: BatchId | undefined;

function makeSummary(id: BatchId): ProductionBatchSummary {
  return {
    id,
    projectId: "project-1",
    name: id.replace("batch-", "Batch ").toUpperCase(),
    status: batchStatuses[id],
    continueOnFailure: false,
    createdAt: "2026-09-02T00:00:00Z",
    updatedAt: "2026-09-02T00:00:00Z",
  };
}

function makeDetail(id: BatchId): ProductionBatchDetail {
  const counts = batchCounts[id];
  const status = batchStatuses[id];
  return {
    ...makeSummary(id),
    total: counts.total,
    pending: status === "READY" ? counts.total - counts.succeeded - counts.failed - counts.cancelled - counts.skipped : 0,
    running: status === "RUNNING" ? Math.max(1, counts.total - counts.succeeded - counts.failed - counts.cancelled - counts.skipped) : 0,
    succeeded: counts.succeeded,
    failed: counts.failed,
    cancelled: counts.cancelled,
    skipped: counts.skipped,
    items: [],
  };
}

function makeReview(id: BatchId): ProductionBatchReviewProductivity {
  return {
    batch: makeDetail(id),
    total: batchCounts[id].total,
    successCount: batchCounts[id].succeeded,
    failedCount: batchCounts[id].failed,
    unreviewedCount: 0,
    approvedCount: 0,
    starredCount: 0,
    regenerateCount: 0,
    rejectedCount: 0,
    items: [],
  };
}

function queueRow(id: BatchId): HTMLElement {
  const row = document.querySelector(`[data-batch-id="${id}"]`);
  if (!(row instanceof HTMLElement)) throw new Error(`missing queue row ${id}`);
  return row;
}

function startButton(id: BatchId): HTMLElement {
  return within(queueRow(id)).getByRole("button", { name: `开始队列 ${id}` });
}

async function flushAsyncWork() {
  await act(async () => {
    for (let index = 0; index < 20; index += 1) await Promise.resolve();
  });
}

function emitTerminalTask(id: BatchId = "batch-a") {
  taskUpdateListener?.({
    id: `task-${id}`,
    projectId: "project-1",
    status: "SUCCEEDED",
    progress: { mode: "step", current: 1, total: 1 },
    createdAt: "2026-09-02T00:00:00Z",
    finishedAt: "2026-09-02T00:00:01Z",
    outputAssetIds: [],
  });
}

async function completeBatch(id: BatchId) {
  batchStatuses[id] = "COMPLETED";
  batchCounts[id] = { ...batchCounts[id], succeeded: batchCounts[id].total, failed: 0, cancelled: 0, skipped: 0 };
  if (activeBatchId === id) {
    activeBatchId = undefined;
  }
  emitTerminalTask(id);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(900);
  });
  await flushAsyncWork();
}

beforeEach(() => {
  vi.useFakeTimers();
  batchStatuses = { "batch-a": "READY", "batch-b": "READY", "batch-c": "READY", "batch-d": "READY" };
  batchCounts = {
    "batch-a": { total: 1, succeeded: 0, failed: 0, cancelled: 0, skipped: 0 },
    "batch-b": { total: 1, succeeded: 0, failed: 0, cancelled: 0, skipped: 0 },
    "batch-c": { total: 1, succeeded: 0, failed: 0, cancelled: 0, skipped: 0 },
    "batch-d": { total: 1, succeeded: 0, failed: 0, cancelled: 0, skipped: 0 },
  };
  activeBatchId = undefined;
  startFailure = undefined;
  taskUpdateListener = undefined;
  Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
  mocks.listShots.mockResolvedValue([]);
  mocks.getProjectWorkflowConfig.mockResolvedValue({ projectId: "project-1", videoModeOverrides: [] });
  mocks.listRecentAssets.mockResolvedValue([]);
  mocks.listPromptLibrary.mockResolvedValue({ items: [], total: 0 });
  mocks.listReferenceAnchors.mockResolvedValue([]);
  mocks.listProductionStructure.mockResolvedValue({ projectId: "project-1", series: [], unassignedShotIds: [] });
  mocks.getProductionBatchRunbook.mockResolvedValue({ projectId: "project-1", rows: [] });
  mocks.listBatchWorkflowPresets.mockResolvedValue([]);
  mocks.listProductionQueues.mockImplementation(async () => ids.map(makeSummary));
  mocks.getProductionAdmissionStatus.mockImplementation(async () => ({
    busy: Boolean(activeBatchId),
    batchId: activeBatchId,
    projectId: activeBatchId ? "project-1" : undefined,
  }));
  mocks.getProductionQueue.mockImplementation(async (_projectId: string, id: BatchId) => makeDetail(id));
  mocks.getProductionQueueOverview.mockImplementation(async () => {
    const values = ids.map((id) => makeDetail(id));
    return {
      totalQueues: values.length,
      runningQueues: values.filter((detail) => detail.status === "RUNNING").length,
      pausedQueues: values.filter((detail) => detail.status === "PAUSED").length,
      completedQueues: values.filter((detail) => detail.status === "COMPLETED").length,
      archivedQueues: 0,
      totalItems: values.reduce((sum, detail) => sum + detail.total, 0),
      pendingItems: values.reduce((sum, detail) => sum + detail.pending, 0),
      activeItems: values.reduce((sum, detail) => sum + detail.running, 0),
      succeededItems: values.reduce((sum, detail) => sum + detail.succeeded, 0),
      failedItems: values.reduce((sum, detail) => sum + detail.failed, 0),
      cancelledItems: values.reduce((sum, detail) => sum + detail.cancelled, 0),
      skippedItems: values.reduce((sum, detail) => sum + detail.skipped, 0),
    } satisfies ProductionQueueOverview;
  });
  mocks.getProductionBatchReviewProductivity.mockImplementation(async (_projectId: string, id: BatchId) => makeReview(id));
  mocks.getAssetMediaUrl.mockImplementation((_projectId: string, assetId: string) => `asset://${assetId}`);
  mocks.startProductionQueue.mockImplementation(async (_projectId: string, id: BatchId) => {
    if (id === startFailure) throw { code: "COMFY_TIMEOUT", message: "start failed" };
    batchStatuses[id] = "RUNNING";
    activeBatchId = id;
    return makeDetail(id);
  });
  mocks.pauseProductionQueue.mockResolvedValue(undefined);
  mocks.requeueProductionQueueItem.mockResolvedValue(undefined);
  mocks.revealProductionReviewAsset.mockResolvedValue(undefined);
  mocks.openProductionReviewOutputFolder.mockResolvedValue(undefined);
  mocks.subscribeTaskUpdates.mockImplementation(async (listener: (task: TaskView) => void) => {
    taskUpdateListener = listener;
    return () => {
      if (taskUpdateListener === listener) taskUpdateListener = undefined;
    };
  });
});

afterEach(() => {
  Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
  vi.useRealTimers();
  cleanup();
  vi.resetAllMocks();
});

describe("ShotWorkspace explicit sequential batch start", () => {
  it("returns the sequential state to idle after the final armed batch completes", async () => {
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await flushAsyncWork();
    screen.getByRole("region", { name: "生产队列" });
    fireEvent.click(startButton("batch-a"));
    await flushAsyncWork();
    fireEvent.click(startButton("batch-b"));
    fireEvent.click(startButton("batch-c"));
    await flushAsyncWork();

    await completeBatch("batch-a");
    await completeBatch("batch-b");
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(3, "project-1", "batch-c");

    await completeBatch("batch-c");
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(3);
    expect(document.querySelector('[data-sequential-status="ACTIVE"]')).toBeNull();
    expect(document.querySelector('[data-sequential-status="PAUSED"]')).toBeNull();
    expect(screen.queryByText(/等待：0 个/)).toBeNull();
  });

  it("starts a new batch immediately after the previous sequence has fully completed", async () => {
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await flushAsyncWork();
    screen.getByRole("region", { name: "生产队列" });
    fireEvent.click(startButton("batch-a"));
    await flushAsyncWork();
    fireEvent.click(startButton("batch-b"));
    fireEvent.click(startButton("batch-c"));
    await flushAsyncWork();

    await completeBatch("batch-a");
    await completeBatch("batch-b");
    await completeBatch("batch-c");
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(3);
    expect(document.querySelector('[data-sequential-status="ACTIVE"]')).toBeNull();

    fireEvent.click(startButton("batch-d"));
    await flushAsyncWork();
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(4, "project-1", "batch-d");
    expect(within(queueRow("batch-d")).getByText("运行中")).toBeTruthy();
    expect(within(queueRow("batch-d")).queryByText(/等待自动开始 #1/)).toBeNull();
  });

  it("clears terminal failed state when there are no armed suffix batches", async () => {
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await flushAsyncWork();
    screen.getByRole("region", { name: "生产队列" });
    fireEvent.click(startButton("batch-a"));
    await flushAsyncWork();

    batchStatuses["batch-a"] = "COMPLETED";
    batchCounts["batch-a"] = { total: 1, succeeded: 0, failed: 1, cancelled: 0, skipped: 0 };
    activeBatchId = undefined;
    emitTerminalTask("batch-a");
    await act(async () => { await vi.advanceTimersByTimeAsync(900); });
    await flushAsyncWork();
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);
    expect(document.querySelector('[data-sequential-status="ACTIVE"]')).toBeNull();
    expect(document.querySelector('[data-sequential-status="PAUSED"]')).toBeNull();
    expect(screen.queryByRole("button", { name: "继续后续" })).toBeNull();

    fireEvent.click(startButton("batch-d"));
    await flushAsyncWork();
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(2, "project-1", "batch-d");
    expect(within(queueRow("batch-d")).getByText("运行中")).toBeTruthy();
  });

  it("runs explicitly started batches in click order and never starts an unarmed batch", async () => {
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await flushAsyncWork();
    const drawer = screen.getByRole("region", { name: "生产队列" });
    fireEvent.click(startButton("batch-a"));
    await flushAsyncWork();
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(1, "project-1", "batch-a");

    fireEvent.click(startButton("batch-b"));
    fireEvent.click(startButton("batch-c"));
    await flushAsyncWork();
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);
    expect(within(queueRow("batch-b")).getByText(/等待自动开始 #1/)).toBeTruthy();
    expect(within(queueRow("batch-c")).getByText(/等待自动开始 #2/)).toBeTruthy();
    expect(within(queueRow("batch-d")).getByRole("button", { name: "开始队列 batch-d" })).toBeTruthy();

    await completeBatch("batch-a");
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(2, "project-1", "batch-b");
    expect(within(queueRow("batch-c")).getByText(/等待自动开始 #1/)).toBeTruthy();
    expect(mocks.startProductionQueue).not.toHaveBeenCalledWith("project-1", "batch-d");

    await completeBatch("batch-b");
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(3, "project-1", "batch-c");
    expect(mocks.startProductionQueue).not.toHaveBeenCalledWith("project-1", "batch-d");
    expect(drawer).toBeTruthy();
  });

  it("waits for the complete multi-item batch truth before advancing", async () => {
    batchCounts["batch-a"] = { total: 8, succeeded: 0, failed: 0, cancelled: 0, skipped: 0 };
    batchCounts["batch-b"] = { total: 1, succeeded: 0, failed: 0, cancelled: 0, skipped: 0 };
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await flushAsyncWork();
    screen.getByRole("region", { name: "生产队列" });
    fireEvent.click(startButton("batch-a"));
    fireEvent.click(startButton("batch-b"));
    await flushAsyncWork();

    batchCounts["batch-a"] = { total: 8, succeeded: 7, failed: 0, cancelled: 0, skipped: 0 };
    activeBatchId = undefined;
    emitTerminalTask("batch-a");
    await act(async () => { await vi.advanceTimersByTimeAsync(900); });
    await flushAsyncWork();
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);

    await completeBatch("batch-a");
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(2, "project-1", "batch-b");
  });

  it("deduplicates repeated intent and supports cancelling one waiting batch", async () => {
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await flushAsyncWork();
    screen.getByRole("region", { name: "生产队列" });
    fireEvent.click(startButton("batch-a"));
    fireEvent.click(startButton("batch-b"));
    await flushAsyncWork();
    expect(within(queueRow("batch-b")).getByRole("button", { name: "取消等待 batch-b" })).toBeTruthy();
    expect(within(queueRow("batch-b")).getAllByText(/等待自动开始 #1/)).toHaveLength(1);
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);

    fireEvent.click(within(queueRow("batch-b")).getByRole("button", { name: "取消等待 batch-b" }));
    await flushAsyncWork();
    expect(within(queueRow("batch-b")).getByRole("button", { name: "开始队列 batch-b" })).toBeTruthy();
    await completeBatch("batch-a");
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);
  });

  it("cancels only the queued suffix while the current batch continues", async () => {
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await flushAsyncWork();
    screen.getByRole("region", { name: "生产队列" });
    fireEvent.click(startButton("batch-a"));
    fireEvent.click(startButton("batch-b"));
    fireEvent.click(startButton("batch-c"));
    await flushAsyncWork();
    fireEvent.click(screen.getByRole("button", { name: "取消后续连续运行" }));
    await flushAsyncWork();
    expect(screen.queryByText(/等待自动开始 #1/)).toBeNull();
    expect(screen.getByText(/当前任务会继续完成。/)).toBeTruthy();

    await completeBatch("batch-a");
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);
    expect(batchStatuses["batch-a"]).toBe("COMPLETED");
  });

  it("pauses on a failed terminal batch and only continues after explicit confirmation", async () => {
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await flushAsyncWork();
    screen.getByRole("region", { name: "生产队列" });
    fireEvent.click(startButton("batch-a"));
    fireEvent.click(startButton("batch-b"));
    await flushAsyncWork();

    batchStatuses["batch-a"] = "COMPLETED";
    batchCounts["batch-a"] = { total: 1, succeeded: 0, failed: 1, cancelled: 0, skipped: 0 };
    activeBatchId = undefined;
    emitTerminalTask("batch-a");
    await act(async () => { await vi.advanceTimersByTimeAsync(900); });
    await flushAsyncWork();
    expect(screen.getByText(/连续运行已暂停/)).toBeTruthy();
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "继续后续" }));
    await flushAsyncWork();
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(2, "project-1", "batch-b");
  });

  it("advances after visibility resume through the existing refresh path", async () => {
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await flushAsyncWork();
    screen.getByRole("region", { name: "生产队列" });
    fireEvent.click(startButton("batch-a"));
    fireEvent.click(startButton("batch-b"));
    await flushAsyncWork();

    Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
    batchStatuses["batch-a"] = "COMPLETED";
    batchCounts["batch-a"] = { total: 1, succeeded: 1, failed: 0, cancelled: 0, skipped: 0 };
    activeBatchId = undefined;
    await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);

    Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
    document.dispatchEvent(new Event("visibilitychange"));
    await flushAsyncWork();
    expect(mocks.startProductionQueue).toHaveBeenNthCalledWith(2, "project-1", "batch-b");
  });

  it("keeps a next batch queued and exposes the error when its start fails", async () => {
    startFailure = "batch-b";
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await flushAsyncWork();
    screen.getByRole("region", { name: "生产队列" });
    fireEvent.click(startButton("batch-a"));
    fireEvent.click(startButton("batch-b"));
    await flushAsyncWork();
    await completeBatch("batch-a");
    await flushAsyncWork();
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(2);
    expect(screen.getAllByText(/ComfyUI 响应超时/).length).toBeGreaterThanOrEqual(2);
    expect(within(queueRow("batch-b")).getByRole("button", { name: "取消等待 batch-b" })).toBeTruthy();
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(2);
  });
});
