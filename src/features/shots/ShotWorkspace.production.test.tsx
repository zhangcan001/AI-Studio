// @vitest-environment jsdom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProductionBatchSummary, ProductionQueueOverview } from "../../types/productionQueue";
import type { ProductionPackageCreateBatchesResult, ProductionPackageInspectionResult } from "../../services/tauriClient";
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
  getProductionQueueOverview: vi.fn(),
  startProductionQueue: vi.fn(),
  pauseProductionQueue: vi.fn(),
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
    getProductionQueueOverview: mocks.getProductionQueueOverview,
    startProductionQueue: mocks.startProductionQueue,
    pauseProductionQueue: mocks.pauseProductionQueue,
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

let queues: ProductionBatchSummary[];

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

beforeEach(() => {
  queues = [];
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
  mocks.getProductionQueueOverview.mockImplementation(async () => ({ ...queueOverview, totalQueues: queues.length, totalItems: queues.length }));
  mocks.startProductionQueue.mockImplementation(async () => {
    queues = [makeQueue("RUNNING")];
    return {};
  });
  mocks.pauseProductionQueue.mockResolvedValue({});
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("ShotWorkspace production package queue integration", () => {
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
});
