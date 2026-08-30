// @vitest-environment jsdom

import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProductionBatchDetail, ProductionBatchSummary, ProductionQueueOverview } from "../../types/productionQueue";
import type { ProductionPackageCreateBatchesResult, ProductionPackageInspectionResult } from "../../services/tauriClient";
import type { ProductionBatchReviewProductivity } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import { ShotWorkspace } from "./ShotWorkspace";

const mocks = vi.hoisted(() => ({
  listShots: vi.fn(),
  listRecentAssets: vi.fn(),
  listPromptLibrary: vi.fn(),
  listReferenceAnchors: vi.fn(),
  listProductionStructure: vi.fn(),
  getProductionBatchRunbook: vi.fn(),
  listBatchWorkflowPresets: vi.fn(),
  pickProductionPackageRoot: vi.fn(),
  inspectProductionPackage: vi.fn(),
  createProductionPackageBatches: vi.fn(),
  listProductionQueues: vi.fn(),
  getProductionQueue: vi.fn(),
  getProductionBatchReviewProductivity: vi.fn(),
  getProductionQueueOverview: vi.fn(),
  startProductionQueue: vi.fn(),
  pauseProductionQueue: vi.fn(),
  requeueProductionQueueItem: vi.fn(),
  revealProductionReviewAsset: vi.fn(),
  openProductionReviewOutputFolder: vi.fn(),
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
    inspectProductionPackage: mocks.inspectProductionPackage,
    createProductionPackageBatches: mocks.createProductionPackageBatches,
    listProductionQueues: mocks.listProductionQueues,
    getProductionQueue: mocks.getProductionQueue,
    getProductionBatchReviewProductivity: mocks.getProductionBatchReviewProductivity,
    getProductionQueueOverview: mocks.getProductionQueueOverview,
    startProductionQueue: mocks.startProductionQueue,
    pauseProductionQueue: mocks.pauseProductionQueue,
    requeueProductionQueueItem: mocks.requeueProductionQueueItem,
    revealProductionReviewAsset: mocks.revealProductionReviewAsset,
    openProductionReviewOutputFolder: mocks.openProductionReviewOutputFolder,
  };
});

vi.mock("./ProjectStructureTree", () => ({
  ProjectStructureTree: () => <div aria-label="项目结构" />,
}));

vi.mock("./ProjectProductionPipeline", () => ({
  ProjectProductionPipeline: () => <div aria-label="项目生产流水线" />,
}));

vi.mock("../production/ProductionBatchRunbookPanel", () => ({
  ProductionBatchRunbookPanel: () => <div aria-label="空生产手册" />,
}));

vi.mock("../production/ProductionMonitor", () => ({
    ProductionMonitor: (props: {
      batch?: { status?: string; items?: Array<{ assetId?: string }> };
      onRetryItem?: (itemId: string) => void | Promise<void>;
      onPlay?: (itemId: string, assetId: string) => void | Promise<void>;
      onOpenFileLocation?: (itemId: string, filePath?: string) => void | Promise<void>;
    }) => (
      <section aria-label="生产监控">
        <strong data-testid="monitor-batch-status">{props.batch?.status ?? "EMPTY"}</strong>
        <span data-testid="monitor-output-asset-ids">{props.batch?.items?.flatMap((item) => item.assetId ?? []).join(",")}</span>
        <button type="button" onClick={() => void props.onRetryItem?.("item-1")}>监控项重试</button>
        <button type="button" onClick={() => void props.onPlay?.("item-1", "asset-success")}>监控资产预览</button>
        <button type="button" onClick={() => void props.onOpenFileLocation?.("item-1", "D:/AIStudio/outputs/success.mp4")}>监控资产位置</button>
      </section>
    ),
}));

let queues: ProductionBatchSummary[];
let batchStatus: ProductionBatchDetail["status"] = "READY";
let review: ProductionBatchReviewProductivity;

const successAsset: AssetView = {
  id: "asset-success",
  assetType: "video",
  category: "generated_video",
  name: "成功视频",
  originalName: "success.mp4",
  mimeType: "video/mp4",
  width: 960,
  height: 544,
  durationMs: 5000,
  fileSize: 100,
  createdAt: "2026-08-29T00:00:03Z",
  sourceTaskId: "task-success",
  thumbnailAvailable: true,
  isFavorite: false,
  tags: [],
};

const queueOverview: ProductionQueueOverview = {
  totalQueues: 0,
  runningQueues: 0,
  pausedQueues: 0,
  completedQueues: 0,
  archivedQueues: 0,
  totalItems: 0,
  pendingItems: 0,
  activeItems: 0,
  succeededItems: 0,
  failedItems: 0,
  cancelledItems: 0,
  skippedItems: 0,
};

const inspection: ProductionPackageInspectionResult = {
  inspectionId: "inspection-uat",
  packageName: "UAT package",
  packageType: "AI_STUDIO_VIDEO_PRODUCTION",
  itemCount: 1,
  readyCount: 1,
  warningCount: 0,
  blockedCount: 0,
  items: [{
    id: "UAT-SH-001",
    name: "UAT shot",
    mode: "FL2VA_IMAGE_TO_VIDEO",
    videoPromptPreview: "A lantern moves through the rain.",
    duration: 5,
    resolution: { width: 960, height: 544 },
    status: "READY",
    warnings: [],
    errors: [],
  }],
};

const created: ProductionPackageCreateBatchesResult = {
  packageName: "UAT package",
  status: "COMPLETE",
  requestedCount: 1,
  createdCount: 1,
  remainingCount: 0,
  remainingItemIds: [],
  batchCount: 1,
  itemCount: 1,
  autoStarted: false,
  batches: [{ batchId: "pbt_uat_001", batchName: "UAT package", itemCount: 1, itemMappings: [] }],
  itemMappings: [],
  warnings: [],
};

function makeQueue(status: ProductionBatchSummary["status"] = "READY"): ProductionBatchSummary {
  return {
    id: "pbt_uat_001",
    projectId: "project-1",
    name: "UAT package",
    status,
    continueOnFailure: false,
    createdAt: "2026-08-29T00:00:00Z",
    updatedAt: "2026-08-29T00:00:00Z",
  };
}

function makeBatchDetail(status: ProductionBatchDetail["status"] = batchStatus): ProductionBatchDetail {
  return {
    ...makeQueue(status),
    total: 1,
    pending: status === "READY" ? 1 : 0,
    running: status === "RUNNING" ? 1 : 0,
    succeeded: status === "COMPLETED" ? 1 : 0,
    failed: 0,
    cancelled: 0,
    skipped: 0,
    items: [{
      id: "item-1",
      ordinal: 0,
      workflowVersionId: "workflow-1",
      recipeId: "recipe-1",
      status: status === "READY" ? "PENDING" : status === "RUNNING" ? "DISPATCHED" : "SUCCEEDED",
      taskId: "task-success",
    }],
  };
}

function makeReview(outputAssets: AssetView[] = []): ProductionBatchReviewProductivity {
  return {
    batch: makeBatchDetail(batchStatus),
    total: 1,
    successCount: outputAssets.length ? 1 : 0,
    failedCount: 0,
    unreviewedCount: outputAssets.length ? 1 : 0,
    approvedCount: 0,
    starredCount: 0,
    regenerateCount: 0,
    rejectedCount: 0,
    items: [{
      itemId: "item-1",
      ordinal: 0,
      taskId: "task-success",
      taskStatus: outputAssets.length ? "SUCCEEDED" : batchStatus === "RUNNING" ? "RUNNING" : "PENDING",
      productionItemStatus: outputAssets.length ? "SUCCEEDED" : batchStatus === "RUNNING" ? "DISPATCHED" : "PENDING",
      reviewStatus: outputAssets.length ? "UNREVIEWED" : "IN_PROGRESS",
      reviewNote: "",
      preferred: true,
      workflowVersionId: "workflow-1",
      recipeId: "recipe-1",
      qualityProfile: "QUALITY",
      createdAt: "2026-08-29T00:00:00Z",
      outputAssets,
      shotId: "shot-1",
      stage: "VIDEO",
      selectedAssetId: outputAssets[0]?.id,
      reviewable: outputAssets.length > 0,
      candidateAssets: outputAssets.map((asset) => ({
        assetId: asset.id,
        assetType: asset.assetType ?? "video",
        name: asset.name,
        mimeType: asset.mimeType,
        width: asset.width,
        height: asset.height,
        thumbnailAvailable: Boolean(asset.thumbnailAvailable),
        taskId: asset.sourceTaskId,
        localPath: "D:/AIStudio/outputs/success.mp4",
        selected: false,
        reviewResult: "UNREVIEWED",
      })),
      context: { snapshotAvailable: true },
    }],
  };
}

beforeEach(() => {
  queues = [];
  batchStatus = "READY";
  review = makeReview();
  mocks.listShots.mockResolvedValue([]);
  mocks.listRecentAssets.mockResolvedValue([]);
  mocks.listPromptLibrary.mockResolvedValue({ items: [], total: 0 });
  mocks.listReferenceAnchors.mockResolvedValue([]);
  mocks.listProductionStructure.mockResolvedValue({ projectId: "project-1", series: [], unassignedShotIds: [] });
  mocks.getProductionBatchRunbook.mockResolvedValue({ projectId: "project-1", rows: [] });
  mocks.listBatchWorkflowPresets.mockResolvedValue([]);
  mocks.pickProductionPackageRoot.mockResolvedValue("C:/uat");
  mocks.inspectProductionPackage.mockResolvedValue(inspection);
  mocks.createProductionPackageBatches.mockImplementation(async () => {
    queues = [makeQueue()];
    return created;
  });
  mocks.listProductionQueues.mockImplementation(async () => queues);
  mocks.getProductionQueue.mockImplementation(async () => makeBatchDetail());
  mocks.getProductionBatchReviewProductivity.mockImplementation(async () => review);
  mocks.getProductionQueueOverview.mockImplementation(async () => ({ ...queueOverview, totalQueues: queues.length, totalItems: queues.length }));
  mocks.startProductionQueue.mockImplementation(async () => {
    batchStatus = "RUNNING";
    queues = [makeQueue("RUNNING")];
    review = makeReview();
    return makeBatchDetail("RUNNING");
  });
  mocks.pauseProductionQueue.mockResolvedValue({});
  mocks.requeueProductionQueueItem.mockResolvedValue(makeBatchDetail("READY"));
  mocks.revealProductionReviewAsset.mockResolvedValue(undefined);
  mocks.openProductionReviewOutputFolder.mockResolvedValue(undefined);
});

afterEach(() => {
  Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
  vi.useRealTimers();
  cleanup();
  vi.resetAllMocks();
});

describe("ShotWorkspace production package queue integration", () => {
  it("reopens a manually collapsed production queue after production package quick create", async () => {
    const user = userEvent.setup();
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);

    const drawer = await screen.findByRole("region", { name: "生产队列" });
    const toggle = drawer.querySelector("button[aria-controls]") as HTMLButtonElement;
    await waitFor(() => expect(toggle.getAttribute("aria-expanded")).toBe("true"));

    await user.click(toggle);
    await waitFor(() => expect(toggle.getAttribute("aria-expanded")).toBe("false"));

    await user.click(await screen.findByRole("button", { name: "选择生产包文件夹" }));
    await waitFor(() => expect(mocks.inspectProductionPackage).toHaveBeenCalledWith("project-1", "C:/uat"));
    await user.click(screen.getByRole("button", { name: "创建并打开生产队列（1 项）" }));
    await screen.findByRole("region", { name: "生产包创建结果" });

    await waitFor(() => expect(toggle.getAttribute("aria-expanded")).toBe("true"));
    expect(drawer.querySelector("[data-batch-id='pbt_uat_001']")?.getAttribute("data-focused")).toBe("true");
    expect(drawer.querySelector("[data-batch-id='pbt_uat_001']")?.getAttribute("data-recently-created")).toBe("true");
    expect(within(drawer).getByText("刚刚创建")).toBeTruthy();
    expect(within(drawer).getByRole("button", { name: "开始队列 pbt_uat_001" })).toBeTruthy();
    expect(mocks.startProductionQueue).not.toHaveBeenCalled();
  });

  it("quick-creates, opens, focuses, and keeps a created generic batch manually startable", async () => {
    const user = userEvent.setup();
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);

    await user.click(await screen.findByRole("button", { name: "选择生产包文件夹" }));
    await waitFor(() => expect(mocks.inspectProductionPackage).toHaveBeenCalledWith("project-1", "C:/uat"));
    await user.click(screen.getByRole("button", { name: "创建并打开生产队列（1 项）" }));
    await screen.findByRole("region", { name: "生产包创建结果" });

    expect(mocks.startProductionQueue).not.toHaveBeenCalled();

    const drawer = await screen.findByRole("region", { name: "生产队列" });
    expect(drawer.querySelector("[data-batch-id='pbt_uat_001']")?.getAttribute("data-focused")).toBe("true");
    expect(drawer.querySelector("[data-batch-id='pbt_uat_001']")?.getAttribute("data-recently-created")).toBe("true");
    expect(within(drawer).getByText("刚刚创建")).toBeTruthy();
    expect(within(drawer).getByText("pbt_uat_001")).toBeTruthy();
    expect(within(drawer).getByRole("button", { name: "开始队列 pbt_uat_001" })).toBeTruthy();
    expect(mocks.startProductionQueue).not.toHaveBeenCalled();

    await user.click(within(drawer).getByRole("button", { name: "开始队列 pbt_uat_001" }));
    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-1", "pbt_uat_001"));
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);
  });

  it("wires Quick Create -> Queue -> manual Start -> RUNNING -> successful asset without auto-start", async () => {
    const user = userEvent.setup();
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);

    await user.click(await screen.findByRole("button", { name: "选择生产包文件夹" }));
    await waitFor(() => expect(mocks.inspectProductionPackage).toHaveBeenCalledWith("project-1", "C:/uat"));
    await user.click(screen.getByRole("button", { name: "创建并打开生产队列（1 项）" }));
    await screen.findByRole("region", { name: "生产包创建结果" });

    expect(mocks.startProductionQueue).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByTestId("monitor-batch-status").textContent).toContain("READY"));
    expect(mocks.getProductionQueue).toHaveBeenCalledWith("project-1", "pbt_uat_001");
    expect(mocks.getProductionBatchReviewProductivity).toHaveBeenCalledWith("project-1", "pbt_uat_001");

    const drawer = await screen.findByRole("region", { name: "生产队列" });
    await user.click(within(drawer).getByRole("button", { name: "开始队列 pbt_uat_001" }));
    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-1", "pbt_uat_001"));
    await waitFor(() => expect(screen.getByTestId("monitor-batch-status").textContent).toContain("RUNNING"));

    batchStatus = "COMPLETED";
    review = makeReview([successAsset]);
    document.dispatchEvent(new Event("visibilitychange"));
    await waitFor(() => expect(screen.getByTestId("monitor-output-asset-ids").textContent).toContain("asset-success"));
    await user.click(screen.getByRole("button", { name: "监控资产预览" }));
    expect(await screen.findByRole("dialog", { name: "成功视频 全图预览" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "监控资产位置" }));
    await waitFor(() => expect(mocks.revealProductionReviewAsset).toHaveBeenCalledWith({
      projectId: "project-1",
      batchId: "pbt_uat_001",
      itemId: "item-1",
      assetId: "asset-success",
    }));
    expect(mocks.startProductionQueue).toHaveBeenCalledTimes(1);
  });

  it("requeues a failed monitor item without starting it", async () => {
    const user = userEvent.setup();
    queues = [makeQueue("READY")];
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);

    await waitFor(() => expect(screen.getByTestId("monitor-batch-status").textContent).toContain("READY"));
    await user.click(screen.getByRole("button", { name: "监控项重试" }));
    await waitFor(() => expect(mocks.requeueProductionQueueItem).toHaveBeenCalledWith("project-1", "pbt_uat_001", "item-1"));
    expect(mocks.startProductionQueue).not.toHaveBeenCalled();
  });

  it("uses one guarded batch poll, pauses while hidden, and stops after terminal completion", async () => {
    vi.useFakeTimers();
    queues = [makeQueue("RUNNING")];
    batchStatus = "RUNNING";
    review = makeReview();
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(1);

    let releaseRefresh: ((detail: ProductionBatchDetail) => void) | undefined;
    let blockRefresh = false;
    mocks.getProductionQueue.mockImplementation(async () => {
      if (blockRefresh) return new Promise((resolve) => { releaseRefresh = resolve; });
      return makeBatchDetail();
    });
    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
    });
    const inFlightCount = mocks.getProductionQueue.mock.calls.length;
    blockRefresh = true;
    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
    });
    const blockedCount = mocks.getProductionQueue.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
    });
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(blockedCount);
    expect(blockedCount).toBe(inFlightCount + 1);

    blockRefresh = false;
    await act(async () => {
      releaseRefresh?.(makeBatchDetail("RUNNING"));
      await Promise.resolve();
      await Promise.resolve();
    });
    Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
    const hiddenCount = mocks.getProductionQueue.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
    });
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(hiddenCount);

    Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(hiddenCount + 1);

    batchStatus = "COMPLETED";
    review = makeReview([successAsset]);
    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
      await Promise.resolve();
    });
    const terminalCount = mocks.getProductionQueue.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(6000);
      await Promise.resolve();
    });
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(terminalCount);
  });
});
