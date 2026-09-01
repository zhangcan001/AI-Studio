// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AssetView } from "../../types/asset";
import type { ProductionBatchDetail, ProductionBatchSummary, ProductionQueueOverview } from "../../types/productionQueue";
import type { ProductionPackageBatchBinding, ProductionPackageDiscoveryPackage, ProductionPackageInspectionResult } from "../../types/productionPackage";
import type { ProductionBatchReviewProductivity } from "../../services/tauriClient";
import type { TaskView } from "../../types/task";
import { ShotWorkspace } from "./ShotWorkspace";

const ids = {
  ep2Package: "package-ep2",
  ep2Root: "C:/season/EP2",
  ep2Item: "ep2-item",
  ep2Batch: "batch-ep2",
  ep10Package: "package-ep10",
  ep10Root: "C:/season/EP10",
  ep10Item: "ep10-item",
  ep10Batch: "batch-ep10",
} as const;

const mocks = vi.hoisted(() => ({
  listShots: vi.fn(),
  listRecentAssets: vi.fn(),
  listPromptLibrary: vi.fn(),
  listReferenceAnchors: vi.fn(),
  listProductionStructure: vi.fn(),
  getProductionBatchRunbook: vi.fn(),
  listBatchWorkflowPresets: vi.fn(),
  pickProductionPackageRoot: vi.fn(),
  discoverProductionPackages: vi.fn(),
  inspectProductionPackage: vi.fn(),
  listProductionPackageBindings: vi.fn(),
  listProductionQueues: vi.fn(),
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
    listRecentAssets: mocks.listRecentAssets,
    listPromptLibrary: mocks.listPromptLibrary,
    listReferenceAnchors: mocks.listReferenceAnchors,
    listProductionStructure: mocks.listProductionStructure,
    getProductionBatchRunbook: mocks.getProductionBatchRunbook,
    listBatchWorkflowPresets: mocks.listBatchWorkflowPresets,
    pickProductionPackageRoot: mocks.pickProductionPackageRoot,
    discoverProductionPackages: mocks.discoverProductionPackages,
    inspectProductionPackage: mocks.inspectProductionPackage,
    listProductionPackageBindings: mocks.listProductionPackageBindings,
    listProductionQueues: mocks.listProductionQueues,
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

vi.mock("../../services/taskEvents", () => ({
  subscribeTaskUpdates: mocks.subscribeTaskUpdates,
}));

vi.mock("./ProjectStructureTree", () => ({
  ProjectStructureTree: () => <div aria-label="项目结构" />,
}));

vi.mock("./ShotListToolbar", () => ({
  ShotListToolbar: () => <div aria-label="镜头搜索和筛选" />,
}));

vi.mock("./ProjectProductionPipeline", () => ({
  ProjectProductionPipeline: () => <div aria-label="项目生产流水线" />,
}));

vi.mock("../production/ProductionPackageWorkspace", () => ({
  ProductionPackageWorkspace: () => <div aria-label="生产包工作区" />,
}));

vi.mock("../production/ProductionBatchRunbookPanel", () => ({
  ProductionBatchRunbookPanel: () => <div aria-label="生产手册" />,
}));

const videoAsset: AssetView = {
  id: "asset-ep2",
  assetType: "video",
  category: "generated_video",
  name: "EP2 成品",
  originalName: "ep2.mp4",
  mimeType: "video/mp4",
  width: 960,
  height: 544,
  durationMs: 5000,
  fileSize: 457274,
  createdAt: "2026-09-01T00:00:00Z",
  sourceTaskId: "task-ep2",
  thumbnailAvailable: true,
  isFavorite: false,
  tags: [],
};

let ep2Status: "RUNNING" | "COMPLETED";
let activeBindings: ProductionPackageBatchBinding[];
let taskUpdateListener: ((task: TaskView) => void) | undefined;

const discoveredPackages: ProductionPackageDiscoveryPackage[] = [
  {
    packageKey: ids.ep2Package,
    packageRoot: ids.ep2Root,
    relativePath: "EP2",
    manifestPath: ids.ep2Root + "/production-package.json",
    manifestSha256: "sha-ep2",
  },
  {
    packageKey: ids.ep10Package,
    packageRoot: ids.ep10Root,
    relativePath: "EP10",
    manifestPath: ids.ep10Root + "/production-package.json",
    manifestSha256: "sha-ep10",
  },
];

const inspections: Record<string, ProductionPackageInspectionResult> = {
  [ids.ep2Root]: {
    inspectionId: "inspection-ep2",
    packageName: "EP2",
    packageType: "AI_STUDIO_VIDEO_PRODUCTION",
    itemCount: 1,
    readyCount: 1,
    warningCount: 0,
    blockedCount: 0,
    manifestSha256: "sha-ep2",
    status: "READY",
    items: [{
      id: ids.ep2Item,
      name: "EP2-SH001",
      mode: "FL2VA_IMAGE_TO_VIDEO",
      videoPromptPreview: "EP2 prompt",
      duration: 5,
      resolution: { width: 960, height: 544 },
      status: "READY",
      warnings: [],
      errors: [],
    }],
  },
  [ids.ep10Root]: {
    inspectionId: "inspection-ep10",
    packageName: "EP10",
    packageType: "AI_STUDIO_VIDEO_PRODUCTION",
    itemCount: 1,
    readyCount: 1,
    warningCount: 0,
    blockedCount: 0,
    manifestSha256: "sha-ep10",
    status: "READY",
    items: [{
      id: ids.ep10Item,
      name: "EP10-SH001",
      mode: "FL2VA_IMAGE_TO_VIDEO",
      videoPromptPreview: "EP10 prompt",
      duration: 5,
      resolution: { width: 960, height: 544 },
      status: "READY",
      warnings: [],
      errors: [],
    }],
  },
};

const bindings: ProductionPackageBatchBinding[] = [
  {
    packageKey: ids.ep2Package,
    packageRoot: ids.ep2Root,
    manifestSha256: "sha-ep2",
    packageId: "package-id-ep2",
    packageName: "EP2",
    batchId: ids.ep2Batch,
    chunkIndex: 0,
    chunkCount: 1,
    packageItemIds: [ids.ep2Item],
    createdAt: "2026-09-01T00:00:00Z",
    sourceKind: "PRODUCTION_PACKAGE",
  },
  {
    packageKey: ids.ep10Package,
    packageRoot: ids.ep10Root,
    manifestSha256: "sha-ep10",
    packageId: "package-id-ep10",
    packageName: "EP10",
    batchId: ids.ep10Batch,
    chunkIndex: 0,
    chunkCount: 1,
    packageItemIds: [ids.ep10Item],
    createdAt: "2026-09-01T00:00:01Z",
    sourceKind: "PRODUCTION_PACKAGE",
  },
];

function makeSummary(batchId: string, name: string, status: ProductionBatchSummary["status"]): ProductionBatchSummary {
  return {
    id: batchId,
    projectId: "project-1",
    name,
    status,
    continueOnFailure: false,
    createdAt: "2026-09-01T00:00:00Z",
    updatedAt: "2026-09-01T00:00:00Z",
  };
}

function makeDetail(
  batchId: string,
  name: string,
  itemId: string,
  status: ProductionBatchDetail["status"],
): ProductionBatchDetail {
  const completed = status === "COMPLETED";
  const running = status === "RUNNING";
  return {
    ...makeSummary(batchId, name, status),
    total: 1,
    pending: status === "READY" ? 1 : 0,
    running: running ? 1 : 0,
    succeeded: completed ? 1 : 0,
    failed: 0,
    cancelled: 0,
    skipped: 0,
    items: [{
      id: itemId,
      ordinal: 0,
      workflowVersionId: "workflow-" + name,
      recipeId: "recipe-" + name,
      status: completed ? "SUCCEEDED" : running ? "DISPATCHED" : "PENDING",
      taskId: name === "EP2" ? "task-ep2" : undefined,
      promptText: name + " prompt",
    }],
  };
}

function makeReview(batchId: string, name: string, itemId: string, status: ProductionBatchDetail["status"]): ProductionBatchReviewProductivity {
  const completed = status === "COMPLETED";
  const running = status === "RUNNING";
  const detail = makeDetail(batchId, name, itemId, status);
  return {
    batch: detail,
    total: 1,
    successCount: completed ? 1 : 0,
    failedCount: 0,
    unreviewedCount: completed ? 1 : 0,
    approvedCount: 0,
    starredCount: 0,
    regenerateCount: 0,
    rejectedCount: 0,
    items: [{
      itemId,
      ordinal: 0,
      taskId: name === "EP2" ? "task-ep2" : undefined,
      taskStatus: completed ? "SUCCEEDED" : running ? "RUNNING" : "PENDING",
      productionItemStatus: completed ? "SUCCEEDED" : running ? "DISPATCHED" : "PENDING",
      reviewStatus: completed ? "UNREVIEWED" : "IN_PROGRESS",
      reviewNote: "",
      preferred: true,
      workflowVersionId: "workflow-" + name,
      recipeId: "recipe-" + name,
      qualityProfile: "QUALITY",
      createdAt: "2026-09-01T00:00:00Z",
      outputAssets: completed && name === "EP2" ? [videoAsset] : [],
      shotId: name + "-shot",
      stage: "VIDEO",
      selectedAssetId: completed && name === "EP2" ? videoAsset.id : undefined,
      reviewable: completed,
      candidateAssets: completed && name === "EP2" ? [{
        assetId: videoAsset.id,
        assetType: "video",
        name: videoAsset.name,
        mimeType: videoAsset.mimeType,
        width: videoAsset.width,
        height: videoAsset.height,
        thumbnailAvailable: true,
        taskId: "task-ep2",
        localPath: "C:/outputs/ep2.mp4",
        selected: true,
        reviewResult: "UNREVIEWED",
      }] : [],
      context: { snapshotAvailable: true },
    }],
  };
}

function makeOverview(): ProductionQueueOverview {
  const completed = ep2Status === "COMPLETED";
  return {
    totalQueues: 2,
    runningQueues: completed ? 0 : 1,
    pausedQueues: 0,
    completedQueues: completed ? 1 : 0,
    archivedQueues: 0,
    totalItems: 2,
    pendingItems: 1,
    activeItems: completed ? 0 : 1,
    succeededItems: completed ? 1 : 0,
    failedItems: 0,
    cancelledItems: 0,
    skippedItems: 0,
  };
}

async function flushAsyncWork() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

async function renderRunningMultiPackageFixture() {
  render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
  await flushAsyncWork();
  fireEvent.click(screen.getByRole("tab", { name: "批量生产包" }));
  fireEvent.click(screen.getByRole("button", { name: "选择根目录" }));
  await flushAsyncWork();
}

function boardRow(packageName: string): HTMLElement {
  return screen.getByRole("row", { name: new RegExp(packageName) });
}

function queueRow(batchId: string): HTMLElement {
  const row = document.querySelector("[data-batch-id='" + batchId + "']");
  if (!(row instanceof HTMLElement)) throw new Error("queue row not found: " + batchId);
  return row;
}

beforeEach(() => {
  vi.useFakeTimers();
  ep2Status = "RUNNING";
  Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
  mocks.listShots.mockResolvedValue([]);
  mocks.listRecentAssets.mockResolvedValue([]);
  mocks.listPromptLibrary.mockResolvedValue({ items: [], total: 0 });
  mocks.listReferenceAnchors.mockResolvedValue([]);
  mocks.listProductionStructure.mockResolvedValue({ projectId: "project-1", series: [], unassignedShotIds: [] });
  mocks.getProductionBatchRunbook.mockResolvedValue({ projectId: "project-1", rows: [] });
  mocks.listBatchWorkflowPresets.mockResolvedValue([]);
  mocks.pickProductionPackageRoot.mockResolvedValue("C:/season");
  mocks.discoverProductionPackages.mockResolvedValue({ rootPath: "C:/season", packages: discoveredPackages });
  mocks.inspectProductionPackage.mockImplementation(async (_projectId: string, packageRoot: string) => inspections[packageRoot]);
  activeBindings = bindings;
  taskUpdateListener = undefined;
  mocks.subscribeTaskUpdates.mockImplementation(async (listener: (task: TaskView) => void) => {
    taskUpdateListener = listener;
    return () => {
      if (taskUpdateListener === listener) taskUpdateListener = undefined;
    };
  });
  mocks.listProductionPackageBindings.mockImplementation(async () => activeBindings);

  mocks.listProductionQueues.mockImplementation(async () => [
    makeSummary(ids.ep2Batch, "EP2", ep2Status),
    makeSummary(ids.ep10Batch, "EP10", "READY"),
  ]);
  mocks.getProductionQueue.mockImplementation(async (_projectId: string, batchId: string) => batchId === ids.ep2Batch
    ? makeDetail(ids.ep2Batch, "EP2", ids.ep2Item, ep2Status)
    : makeDetail(ids.ep10Batch, "EP10", ids.ep10Item, "READY"));
  mocks.getProductionQueueOverview.mockImplementation(async () => makeOverview());
  mocks.getProductionBatchReviewProductivity.mockImplementation(async (_projectId: string, batchId: string) => batchId === ids.ep2Batch
    ? makeReview(ids.ep2Batch, "EP2", ids.ep2Item, ep2Status)
    : makeReview(ids.ep10Batch, "EP10", ids.ep10Item, "READY"));
  mocks.getAssetMediaUrl.mockImplementation((_projectId: string, assetId: string) => "asset://" + assetId);
  mocks.startProductionQueue.mockResolvedValue(makeDetail(ids.ep2Batch, "EP2", ids.ep2Item, "RUNNING"));
  mocks.pauseProductionQueue.mockResolvedValue(undefined);
  mocks.requeueProductionQueueItem.mockResolvedValue(makeDetail(ids.ep2Batch, "EP2", ids.ep2Item, "READY"));
  mocks.revealProductionReviewAsset.mockResolvedValue(undefined);
  mocks.openProductionReviewOutputFolder.mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.resetAllMocks();
});

describe("ShotWorkspace multi-package completion convergence", () => {
  it("converges a running multi-package batch to completed without manual refresh", async () => {
    let holdNextEp2Detail = false;
    let releaseStaleDetail: ((detail: ProductionBatchDetail) => void) | undefined;
    const staleDetail = new Promise<ProductionBatchDetail>((resolve) => {
      releaseStaleDetail = resolve;
    });
    mocks.getProductionQueue.mockImplementation(async (_projectId: string, batchId: string) => {
      if (batchId === ids.ep2Batch && holdNextEp2Detail) {
        holdNextEp2Detail = false;
        return staleDetail;
      }
      return batchId === ids.ep2Batch
        ? makeDetail(ids.ep2Batch, "EP2", ids.ep2Item, ep2Status)
        : makeDetail(ids.ep10Batch, "EP10", ids.ep10Item, "READY");
    });

    await renderRunningMultiPackageFixture();

    expect(within(boardRow("EP2")).getByText("运行中")).toBeTruthy();
    expect(within(queueRow(ids.ep2Batch)).getByText("运行中")).toBeTruthy();
    expect(screen.getByTestId("production-monitor").querySelector(".production-monitor-batch-status")?.textContent).toContain("生成中");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(4000);
    });
    holdNextEp2Detail = true;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    await flushAsyncWork();
    expect(releaseStaleDetail).toBeTypeOf("function");

    ep2Status = "COMPLETED";
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    releaseStaleDetail?.(makeDetail(ids.ep2Batch, "EP2", ids.ep2Item, "RUNNING"));
    await flushAsyncWork();

    expect(within(boardRow("EP2")).getByText("已完成")).toBeTruthy();
    expect(within(boardRow("EP2")).getByLabelText("100%，1/1")).toBeTruthy();
    expect(within(queueRow(ids.ep2Batch)).getByText("已完成")).toBeTruthy();
    const monitor = screen.getByTestId("production-monitor");
    expect(monitor.querySelector(".production-monitor-batch-status")?.textContent).toContain("已完成");
    expect(within(monitor).getByText("成品记录可用")).toBeTruthy();

  });

  it("refreshes the board immediately when visibility returns after completion", async () => {
    await renderRunningMultiPackageFixture();

    Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
    ep2Status = "COMPLETED";
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    await flushAsyncWork();
    expect(within(boardRow("EP2")).getByText("运行中")).toBeTruthy();

    Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await flushAsyncWork();

    expect(within(boardRow("EP2")).getByText("已完成")).toBeTruthy();
    expect(within(boardRow("EP2")).getByLabelText("100%，1/1")).toBeTruthy();
  });

  it("converges all production surfaces after a terminal task event", async () => {
    await renderRunningMultiPackageFixture();

    expect(taskUpdateListener).toBeTypeOf("function");
    ep2Status = "COMPLETED";
    taskUpdateListener?.({
      id: "task-ep2",
      projectId: "project-1",
      status: "SUCCEEDED",
      progress: { mode: "step", current: 1, total: 1 },
      createdAt: "2026-09-01T00:00:00Z",
      finishedAt: "2026-09-01T00:00:01Z",
      outputAssetIds: [videoAsset.id],
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(900);
    });
    await flushAsyncWork();

    expect(within(boardRow("EP2")).getByText("已完成")).toBeTruthy();
    expect(within(boardRow("EP2")).getByLabelText("100%，1/1")).toBeTruthy();
    expect(within(queueRow(ids.ep2Batch)).getByText("已完成")).toBeTruthy();
    expect(screen.getByTestId("production-monitor").querySelector(".production-monitor-batch-status")?.textContent).toContain("已完成");
  });

  it("production queue summary converges after the running batch completes", async () => {
    await renderRunningMultiPackageFixture();

    ep2Status = "COMPLETED";
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    await flushAsyncWork();

    const drawer = screen.getByRole("region", { name: "生产队列" });
    expect(within(queueRow(ids.ep2Batch)).getByText("已完成")).toBeTruthy();
    expect(drawer.querySelector("[aria-label='生产队列摘要']")?.textContent).toContain("0");
  });

  it("keeps the production monitor converged with the completed video asset", async () => {
    await renderRunningMultiPackageFixture();

    ep2Status = "COMPLETED";
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    await flushAsyncWork();

    const monitor = screen.getByTestId("production-monitor");
    expect(monitor.querySelector(".production-monitor-batch-status")?.textContent).toContain("已完成");
    expect(within(monitor).getByText("成品记录可用")).toBeTruthy();
    expect(within(monitor).getByText("播放")).toBeTruthy();
    expect(within(monitor).getByText("打开文件位置")).toBeTruthy();
  });

  it("stops production surface polling after all created batches reach terminal state", async () => {
    activeBindings = [bindings[0]];
    await renderRunningMultiPackageFixture();

    ep2Status = "COMPLETED";
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    await flushAsyncWork();

    const detailCallsAfterCompletion = mocks.getProductionQueue.mock.calls.length;
    const queueCallsAfterCompletion = mocks.listProductionQueues.mock.calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15000);
    });
    await flushAsyncWork();

    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(detailCallsAfterCompletion);
    expect(mocks.listProductionQueues).toHaveBeenCalledTimes(queueCallsAfterCompletion);
  });
});
