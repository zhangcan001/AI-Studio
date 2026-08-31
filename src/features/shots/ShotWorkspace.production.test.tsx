// @vitest-environment jsdom

import { useState } from "react";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProductionBatchDetail, ProductionBatchSummary, ProductionQueueOverview } from "../../types/productionQueue";
import type { ProductionPackageCreateBatchesResult, ProductionPackageInspectionResult } from "../../services/tauriClient";
import type { ProductionBatchReviewProductivity } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { ProductionPackageBatchBinding } from "../../types/productionPackage";
import { buildLocalDeliveryManifest, ShotWorkspace } from "./ShotWorkspace";

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
  createProductionPackageBatches: vi.fn(),
  listProductionPackageBindings: vi.fn(),
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

const multiPackageTestKeys = vi.hoisted(() => ({
  ready: "package-key-ready",
  warning: "package-key-warning",
  blocked: "package-key-blocked",
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
    createProductionPackageBatches: mocks.createProductionPackageBatches,
    listProductionPackageBindings: mocks.listProductionPackageBindings,
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

vi.mock("../production/MultiPackageProductionBoard", () => {
  type TestBoardProps = {
    packages?: Array<{
      packageKey: string;
      packageName: string;
      status: string;
      canCreate?: boolean;
      boundItemCount?: number;
      remainingCount?: number;
      remainingReadyCount?: number;
      remainingWarningCount?: number;
      remainingBlockedCount?: number;
    }>;
    onChooseRoot?: () => void | Promise<void>;
    onCreateSelected?: (keys: string[]) => void | Promise<void>;
    onReinspect?: (packageKey: string) => void;
    onHandleWarning?: (packageKey: string) => void;
  };
  return {
    MultiPackageProductionBoard: (props: TestBoardProps) => {
      const [error, setError] = useState<string>();
      const submit = (keys: string[]) => {
        void Promise.resolve(props.onCreateSelected?.(keys)).catch((reason: unknown) => {
          setError(reason instanceof Error ? reason.message : String(reason));
        });
      };
      return (
        <section aria-label="测试多生产包看板">
          <button type="button" onClick={() => void props.onChooseRoot?.()}>选择批量根目录</button>
          <button type="button" onClick={() => submit([multiPackageTestKeys.ready])}>程序化提交 READY 包</button>
          <button type="button" onClick={() => submit([multiPackageTestKeys.warning])}>程序化提交 WARNING 包</button>
          <button type="button" onClick={() => submit([multiPackageTestKeys.blocked])}>程序化提交 BLOCKED 包</button>
          {props.packages?.map((item) => (
            <article key={item.packageKey} data-testid={"board-package-" + item.packageKey}>
              <strong data-testid={"board-status-" + item.packageKey}>{item.status}</strong>
              <span data-testid={"board-bound-" + item.packageKey}>{item.boundItemCount ?? 0}</span>
              <span data-testid={"board-remaining-" + item.packageKey}>{item.remainingCount ?? 0}</span>
              <span data-testid={"board-ready-" + item.packageKey}>{item.remainingReadyCount ?? 0}</span>
              <span data-testid={"board-warning-" + item.packageKey}>{item.remainingWarningCount ?? 0}</span>
              <span data-testid={"board-blocked-" + item.packageKey}>{item.remainingBlockedCount ?? 0}</span>
              <input type="checkbox" aria-label={"选择 " + item.packageName} checked={Boolean(item.canCreate)} disabled={!item.canCreate} readOnly />
              <button type="button" onClick={() => submit([item.packageKey])} disabled={!item.canCreate}>创建 {item.packageName}</button>
              <button type="button" onClick={() => props.onReinspect?.(item.packageKey)}>重新检查 {item.packageName}</button>
              <button type="button" onClick={() => props.onHandleWarning?.(item.packageKey)}>在单生产包中处理 {item.packageName}</button>
            </article>
          ))}
          {error && <p role="alert">{error}</p>}
        </section>
      );
    },
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
      onExportManifest?: () => void | Promise<void>;
    }) => (
      <section aria-label="生产监控">
        <strong data-testid="monitor-batch-status">{props.batch?.status ?? "EMPTY"}</strong>
        <span data-testid="monitor-output-asset-ids">{props.batch?.items?.flatMap((item) => item.assetId ?? []).join(",")}</span>
        <button type="button" onClick={() => void props.onRetryItem?.("item-1")}>监控项重试</button>
        <button type="button" onClick={() => void props.onPlay?.("item-1", "asset-success")}>监控资产预览</button>
        <button type="button" onClick={() => void props.onOpenFileLocation?.("item-1", "D:/AIStudio/outputs/success.mp4")}>监控资产位置</button>
        <button type="button" onClick={() => void props.onExportManifest?.()}>监控导出成品清单</button>
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

const multiPackageDiscovery = {
  rootPath: "C:/season",
  packages: [
    { packageKey: multiPackageTestKeys.ready, packageRoot: "C:/season/ep01", relativePath: "ep01", manifestPath: "C:/season/ep01/production-package.json", manifestSha256: "sha-ready" },
    { packageKey: multiPackageTestKeys.warning, packageRoot: "C:/season/ep02", relativePath: "ep02", manifestPath: "C:/season/ep02/production-package.json", manifestSha256: "sha-warning" },
    { packageKey: multiPackageTestKeys.blocked, packageRoot: "C:/season/ep03", relativePath: "ep03", manifestPath: "C:/season/ep03/production-package.json", manifestSha256: "sha-blocked" },
  ],
};

function makeMultiPackageInspection(
  packageName: string,
  inspectionId: string,
  manifestSha256: string,
  statuses: Array<"READY" | "WARNING" | "BLOCKED">,
): ProductionPackageInspectionResult {
  const readyCount = statuses.filter((status) => status === "READY").length;
  const warningCount = statuses.filter((status) => status === "WARNING").length;
  const blockedCount = statuses.filter((status) => status === "BLOCKED").length;
  return {
    ...inspection,
    inspectionId,
    packageName,
    manifestSha256,
    itemCount: statuses.length,
    readyCount,
    warningCount,
    blockedCount,
    status: warningCount ? "WARNING" : blockedCount ? "BLOCKED" : "READY",
    items: statuses.map((status, index) => ({
      ...inspection.items[0],
      id: `${inspectionId}-item-${index + 1}`,
      name: `${packageName} 镜头 ${index + 1}`,
      status,
      warnings: status === "WARNING" ? ["需要人工确认"] : [],
      errors: status === "BLOCKED" ? ["存在阻塞问题"] : [],
    })),
  };
}

function makePackageBinding(overrides: Partial<ProductionPackageBatchBinding> = {}): ProductionPackageBatchBinding {
  return {
    packageKey: multiPackageTestKeys.ready,
    packageRoot: "C:/season/ep01",
    manifestSha256: "sha-ready",
    packageId: "package-ready",
    packageName: "READY package",
    batchId: "batch-ready",
    chunkIndex: 0,
    chunkCount: 1,
    packageItemIds: [],
    createdAt: "2026-08-31T00:00:00Z",
    sourceKind: "PRODUCTION_PACKAGE",
    ...overrides,
  };
}

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

function setupMultiPackageInspectionMocks() {
  const inspections: Record<string, ProductionPackageInspectionResult> = {
    "C:/season/ep01": makeMultiPackageInspection("READY package", "inspection-ready", "sha-ready", ["READY", "READY", "READY"]),
    "C:/season/ep02": makeMultiPackageInspection("WARNING package", "inspection-warning", "sha-warning", ["READY", "WARNING", "READY"]),
    "C:/season/ep03": makeMultiPackageInspection("BLOCKED package", "inspection-blocked", "sha-blocked", ["READY", "BLOCKED"]),
  };
  mocks.discoverProductionPackages.mockResolvedValue(multiPackageDiscovery);
  mocks.inspectProductionPackage.mockImplementation(async (_projectId: string, packageRoot: string) => {
    const result = inspections[packageRoot];
    if (!result) throw new Error(`未知生产包：${packageRoot}`);
    return result;
  });
}

function setupSingleMultiPackageFixture(
  discoveredPackage: (typeof multiPackageDiscovery.packages)[number],
  packageInspection: ProductionPackageInspectionResult,
  bindings: ProductionPackageBatchBinding[],
) {
  mocks.discoverProductionPackages.mockResolvedValue({
    rootPath: "C:/season",
    packages: [discoveredPackage],
  });
  mocks.inspectProductionPackage.mockResolvedValue(packageInspection);
  mocks.listProductionPackageBindings.mockResolvedValue(bindings);
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
  mocks.discoverProductionPackages.mockResolvedValue({ rootPath: "C:/season", packages: [] });
  mocks.inspectProductionPackage.mockResolvedValue(inspection);
  mocks.createProductionPackageBatches.mockImplementation(async () => {
    queues = [makeQueue()];
    return created;
  });
  mocks.listProductionPackageBindings.mockResolvedValue([]);
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
  it("blocks programmatic WARNING and BLOCKED package creation at the Host", async () => {
    const user = userEvent.setup();
    setupMultiPackageInspectionMocks();
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);

    await user.click(await screen.findByRole("tab", { name: "批量生产包" }));
    await user.click(screen.getByRole("button", { name: "选择批量根目录" }));
    await waitFor(() => expect(mocks.discoverProductionPackages).toHaveBeenCalledWith("C:/uat"));

    await user.click(screen.getByRole("button", { name: "程序化提交 WARNING 包" }));
    await waitFor(() => expect(screen.getByText(/该生产包包含需要人工确认的警告镜头，请先在单生产包中处理。/)).toBeTruthy());
    expect(mocks.createProductionPackageBatches).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "程序化提交 BLOCKED 包" }));
    await waitFor(() => expect(screen.getByText(/该生产包包含阻塞项目，不能批量创建。/)).toBeTruthy());
    expect(mocks.createProductionPackageBatches).not.toHaveBeenCalled();
    expect(mocks.inspectProductionPackage).toHaveBeenCalledTimes(5);
  });

  it("re-inspects a READY package, creates only READY items, and never starts the queue", async () => {
    const user = userEvent.setup();
    setupMultiPackageInspectionMocks();
    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);

    await user.click(await screen.findByRole("tab", { name: "批量生产包" }));
    await user.click(screen.getByRole("button", { name: "选择批量根目录" }));
    await waitFor(() => expect(mocks.discoverProductionPackages).toHaveBeenCalledWith("C:/uat"));
    await user.click(screen.getByRole("button", { name: "程序化提交 READY 包" }));

    await waitFor(() => expect(mocks.createProductionPackageBatches).toHaveBeenCalledTimes(1));
    expect(mocks.createProductionPackageBatches).toHaveBeenCalledWith(
      "inspection-ready",
      ["inspection-ready-item-1", "inspection-ready-item-2", "inspection-ready-item-3"],
    );
    expect(mocks.inspectProductionPackage).toHaveBeenCalledTimes(4);
    expect(mocks.startProductionQueue).not.toHaveBeenCalled();
  });

  it("rebuilds PARTIAL state from discovery, fresh inspection, and durable bindings after restart", async () => {
    const user = userEvent.setup();
    const partialInspection = makeMultiPackageInspection(
      "Partial READY package",
      "inspection-partial",
      "sha-ready",
      Array.from({ length: 150 }, () => "READY" as const),
    );
    const existingBinding = makePackageBinding({
      packageItemIds: [...partialInspection.items.slice(0, 100).map((item) => item.id), "stale-old-item"],
    });
    setupSingleMultiPackageFixture(multiPackageDiscovery.packages[0], partialInspection, [existingBinding]);

    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await user.click(await screen.findByRole("tab", { name: "批量生产包" }));
    await user.click(screen.getByRole("button", { name: "选择批量根目录" }));

    await waitFor(() => expect(screen.getByTestId("board-status-" + multiPackageTestKeys.ready).textContent).toBe("PARTIAL"));
    expect(screen.getByTestId("board-bound-" + multiPackageTestKeys.ready).textContent).toBe("100");
    expect(screen.getByTestId("board-remaining-" + multiPackageTestKeys.ready).textContent).toBe("50");
    const checkbox = screen.getByRole("checkbox", { name: "选择 Partial READY package" }) as HTMLInputElement;
    expect(checkbox.disabled).toBe(false);
    expect(checkbox.checked).toBe(true);
    expect(mocks.createProductionPackageBatches).not.toHaveBeenCalled();
  });

  it("resumes a PARTIAL package with exactly the remaining READY item IDs", async () => {
    const user = userEvent.setup();
    const partialInspection = makeMultiPackageInspection(
      "Partial READY package",
      "inspection-partial-resume",
      "sha-ready",
      Array.from({ length: 150 }, () => "READY" as const),
    );
    const existingBinding = makePackageBinding({
      packageItemIds: [...partialInspection.items.slice(0, 100).map((item) => item.id), "stale-old-item"],
    });
    setupSingleMultiPackageFixture(multiPackageDiscovery.packages[0], partialInspection, [existingBinding]);
    mocks.createProductionPackageBatches.mockResolvedValue({
      ...created,
      packageName: "Partial READY package",
      requestedCount: 50,
      createdCount: 50,
      itemCount: 50,
      batches: [{ batchId: "batch-resume", batchName: "Partial READY package", itemCount: 50, itemMappings: [] }],
    });

    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await user.click(await screen.findByRole("tab", { name: "批量生产包" }));
    await user.click(screen.getByRole("button", { name: "选择批量根目录" }));
    await waitFor(() => expect(screen.getByTestId("board-status-" + multiPackageTestKeys.ready).textContent).toBe("PARTIAL"));
    await user.click(screen.getByRole("button", { name: "创建 Partial READY package" }));

    await waitFor(() => expect(mocks.createProductionPackageBatches).toHaveBeenCalledTimes(1));
    expect(mocks.createProductionPackageBatches).toHaveBeenCalledWith(
      "inspection-partial-resume",
      partialInspection.items.slice(100).map((item) => item.id),
    );
    expect(mocks.startProductionQueue).not.toHaveBeenCalled();
  });

  it("keeps PARTIAL packages with remaining WARNING items disabled for bulk create", async () => {
    const user = userEvent.setup();
    const statuses: Array<"READY" | "WARNING" | "BLOCKED"> = [
      ...Array.from({ length: 100 }, () => "READY" as const),
      ...Array.from({ length: 40 }, () => "READY" as const),
      ...Array.from({ length: 10 }, () => "WARNING" as const),
    ];
    const partialWarningInspection = makeMultiPackageInspection(
      "Partial WARNING package",
      "inspection-partial-warning",
      "sha-warning",
      statuses,
    );
    const existingBinding = makePackageBinding({
      packageKey: multiPackageTestKeys.warning,
      packageRoot: "C:/season/ep02",
      manifestSha256: "sha-warning",
      packageName: "Partial WARNING package",
      packageItemIds: partialWarningInspection.items.slice(0, 100).map((item) => item.id),
    });
    setupSingleMultiPackageFixture(multiPackageDiscovery.packages[1], partialWarningInspection, [existingBinding]);

    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await user.click(await screen.findByRole("tab", { name: "批量生产包" }));
    await user.click(screen.getByRole("button", { name: "选择批量根目录" }));
    await waitFor(() => expect(screen.getByTestId("board-status-" + multiPackageTestKeys.warning).textContent).toBe("PARTIAL"));
    expect(screen.getByTestId("board-ready-" + multiPackageTestKeys.warning).textContent).toBe("40");
    expect(screen.getByTestId("board-warning-" + multiPackageTestKeys.warning).textContent).toBe("10");
    const checkbox = screen.getByRole("checkbox", { name: "选择 Partial WARNING package" }) as HTMLInputElement;
    expect(checkbox.checked).toBe(false);
    expect(checkbox.disabled).toBe(true);
    expect((screen.getByRole("button", { name: "创建 Partial WARNING package" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole("button", { name: "在单生产包中处理 Partial WARNING package" })).toBeTruthy();
    expect(mocks.createProductionPackageBatches).not.toHaveBeenCalled();
  });

  it("restarts an all-bound package as COMPLETED and never creates a duplicate batch", async () => {
    const user = userEvent.setup();
    const completeInspection = makeMultiPackageInspection(
      "Completed package",
      "inspection-completed",
      "sha-ready",
      Array.from({ length: 150 }, () => "READY" as const),
    );
    const existingBinding = makePackageBinding({
      packageName: "Completed package",
      batchId: "batch-completed",
      packageItemIds: completeInspection.items.map((item) => item.id),
    });
    setupSingleMultiPackageFixture(multiPackageDiscovery.packages[0], completeInspection, [existingBinding]);
    mocks.getProductionQueue.mockImplementation(async (_projectId: string, batchId: string) => ({
      ...makeBatchDetail("COMPLETED"),
      id: batchId,
      name: "Completed package",
      total: 150,
      pending: 0,
      running: 0,
      succeeded: 150,
      failed: 0,
      cancelled: 0,
      skipped: 0,
      items: [],
    }));

    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await user.click(await screen.findByRole("tab", { name: "批量生产包" }));
    await user.click(screen.getByRole("button", { name: "选择批量根目录" }));
    await waitFor(() => expect(screen.getByTestId("board-status-" + multiPackageTestKeys.ready).textContent).toBe("COMPLETED"));
    expect((screen.getByRole("checkbox", { name: "选择 Completed package" }) as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "创建 Completed package" }) as HTMLButtonElement).disabled).toBe(true);
    expect(mocks.createProductionPackageBatches).not.toHaveBeenCalled();
  });

  it("does not join V1 bindings into a V2 package with the same display path", async () => {
    const user = userEvent.setup();
    const v2Package = {
      ...multiPackageDiscovery.packages[0],
      packageKey: "package-key-v2",
      manifestSha256: "sha-v2",
      manifestPath: "C:/season/ep01/production-package.json",
    };
    const v2Inspection = makeMultiPackageInspection(
      "V2 package",
      "inspection-v2",
      "sha-v2",
      ["READY", "READY", "READY"],
    );
    const v1Binding = makePackageBinding({
      packageKey: multiPackageTestKeys.ready,
      packageRoot: "C:/season/ep01",
      manifestSha256: "sha-ready",
      packageName: "V1 package",
      packageItemIds: v2Inspection.items.map((item) => item.id),
    });
    setupSingleMultiPackageFixture(v2Package, v2Inspection, [v1Binding]);

    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await user.click(await screen.findByRole("tab", { name: "批量生产包" }));
    await user.click(screen.getByRole("button", { name: "选择批量根目录" }));
    await waitFor(() => expect(screen.getByTestId("board-status-package-key-v2").textContent).toBe("READY"));
    expect(screen.getByTestId("board-bound-package-key-v2").textContent).toBe("0");
    expect(screen.getByTestId("board-remaining-package-key-v2").textContent).toBe("3");
    expect((screen.getByRole("checkbox", { name: "选择 V2 package" }) as HTMLInputElement).disabled).toBe(false);
    await user.click(screen.getByRole("button", { name: "创建 V2 package" }));
    await waitFor(() => expect(mocks.createProductionPackageBatches).toHaveBeenCalledWith(
      "inspection-v2",
      v2Inspection.items.map((item) => item.id),
    ));
  });

  it("uses a new inspection session after CREATE_FAILED re-inspection", async () => {
    const user = userEvent.setup();
    const oldInspection = makeMultiPackageInspection("Reinspect package", "inspection-old", "sha-ready", ["READY"]);
    const newInspection = makeMultiPackageInspection("Reinspect package", "inspection-new", "sha-ready", ["READY"]);
    setupSingleMultiPackageFixture(multiPackageDiscovery.packages[0], oldInspection, []);
    mocks.createProductionPackageBatches
      .mockRejectedValueOnce(new Error("old inspection consumed"))
      .mockResolvedValueOnce(created);

    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await user.click(await screen.findByRole("tab", { name: "批量生产包" }));
    await user.click(screen.getByRole("button", { name: "选择批量根目录" }));
    await waitFor(() => expect(screen.getByTestId("board-status-" + multiPackageTestKeys.ready).textContent).toBe("READY"));
    await user.click(screen.getByRole("button", { name: "创建 Reinspect package" }));
    await waitFor(() => expect(screen.getByText(/old inspection consumed/)).toBeTruthy());

    mocks.inspectProductionPackage.mockResolvedValue(newInspection);
    await user.click(screen.getByRole("button", { name: "重新检查 Reinspect package" }));
    await waitFor(() => expect(mocks.inspectProductionPackage).toHaveBeenCalledTimes(3));
    await user.click(screen.getByRole("button", { name: "创建 Reinspect package" }));
    await waitFor(() => expect(mocks.createProductionPackageBatches).toHaveBeenCalledTimes(2));
    expect(mocks.createProductionPackageBatches).toHaveBeenLastCalledWith(
      "inspection-new",
      ["inspection-new-item-1"],
    );
    expect(mocks.createProductionPackageBatches.mock.calls[0]).toEqual([
      "inspection-old",
      ["inspection-old-item-1"],
    ]);
  });

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

  it("rejects a delivery manifest when monitor, review, and selected batch IDs disagree", () => {
    const detail = makeBatchDetail("COMPLETED");
    const reviewForDetail = { ...makeReview([successAsset]), batch: detail };

    expect(buildLocalDeliveryManifest(detail, reviewForDetail, detail.id)).toMatchObject({
      batchId: detail.id,
      total: 1,
      items: [{ itemId: "item-1", videoAssetId: "asset-success" }],
    });
    expect(buildLocalDeliveryManifest(detail, reviewForDetail, "pbt_other")).toBeUndefined();
    expect(buildLocalDeliveryManifest(
      detail,
      { ...reviewForDetail, batch: { ...detail, id: "pbt_stale" } },
      detail.id,
    )).toBeUndefined();
  });

  it("refreshes the selected batch before exporting a delivery manifest", async () => {
    const user = userEvent.setup();
    queues = [makeQueue("COMPLETED")];
    batchStatus = "COMPLETED";
    const staleDetail = { ...makeBatchDetail("COMPLETED"), id: "pbt_stale" };
    const currentDetail = makeBatchDetail("COMPLETED");
    const staleReview = { ...makeReview([successAsset]), batch: staleDetail };
    const currentReview = { ...makeReview([successAsset]), batch: currentDetail };
    let queueCall = 0;
    let reviewCall = 0;
    mocks.getProductionQueue.mockImplementation(async () => queueCall++ === 0 ? staleDetail : currentDetail);
    mocks.getProductionBatchReviewProductivity.mockImplementation(async () => reviewCall++ === 0 ? staleReview : currentReview);

    let manifestBlob: Blob | undefined;
    Object.defineProperty(URL, "createObjectURL", {
      configurable: true,
      value: vi.fn((blob: Blob) => {
        manifestBlob = blob;
        return "blob:uat-manifest";
      }),
    });
    Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: vi.fn() });

    render(<ShotWorkspace projectId="project-1" catalog={[]} mode="production" />);
    await waitFor(() => expect(mocks.getProductionBatchReviewProductivity).toHaveBeenCalled());
    await user.click(screen.getByRole("button", { name: "监控导出成品清单" }));

    await waitFor(() => expect(manifestBlob).toBeDefined());
    const manifest = JSON.parse(await manifestBlob!.text()) as { batchId: string; items: Array<{ itemId: string }> };
    expect(manifest.batchId).toBe("pbt_uat_001");
    expect(manifest.items).toHaveLength(1);
    expect(manifest.items[0].itemId).toBe("item-1");
    expect(mocks.getProductionQueue).toHaveBeenLastCalledWith("project-1", "pbt_uat_001");
    expect(mocks.getProductionBatchReviewProductivity).toHaveBeenLastCalledWith("project-1", "pbt_uat_001");
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
