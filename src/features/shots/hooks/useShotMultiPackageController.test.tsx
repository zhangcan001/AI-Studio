// @vitest-environment jsdom

import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProductionBatchDetail } from "../../../types/productionQueue";
import type {
  ProductionPackageBatchBinding,
  ProductionPackageDiscoveryPackage,
  ProductionPackageInspectionResult,
} from "../../../types/productionPackage";
import type { ProductionPackageCreateBatchesResult } from "../../../services/tauriClient";
import { useShotMultiPackageController } from "./useShotMultiPackageController";

const mocks = vi.hoisted(() => ({
  pickProductionPackageRoot: vi.fn(),
  discoverProductionPackages: vi.fn(),
  inspectProductionPackage: vi.fn(),
  listProductionPackageBindings: vi.fn(),
  getProductionQueue: vi.fn(),
  createProductionPackageBatches: vi.fn(),
  reloadProductionQueues: vi.fn(),
  onError: vi.fn(),
  onNotice: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return {
    ...actual,
    pickProductionPackageRoot: mocks.pickProductionPackageRoot,
    discoverProductionPackages: mocks.discoverProductionPackages,
    inspectProductionPackage: mocks.inspectProductionPackage,
    listProductionPackageBindings: mocks.listProductionPackageBindings,
    getProductionQueue: mocks.getProductionQueue,
    createProductionPackageBatches: mocks.createProductionPackageBatches,
  };
});

const packages: ProductionPackageDiscoveryPackage[] = [
  {
    packageKey: "package-a",
    packageRoot: "C:/season/EP01",
    relativePath: "EP01",
    manifestPath: "C:/season/EP01/production-package.json",
    manifestSha256: "sha-a",
  },
  {
    packageKey: "package-b",
    packageRoot: "C:/season/EP02",
    relativePath: "EP02",
    manifestPath: "C:/season/EP02/production-package.json",
    manifestSha256: "sha-b",
  },
];

function makeInspection(
  packageName: string,
  inspectionId: string,
  manifestSha256: string,
  statuses: Array<"READY" | "WARNING" | "BLOCKED">,
): ProductionPackageInspectionResult {
  return {
    inspectionId,
    packageName,
    itemCount: statuses.length,
    readyCount: statuses.filter((status) => status === "READY").length,
    warningCount: statuses.filter((status) => status === "WARNING").length,
    blockedCount: statuses.filter((status) => status === "BLOCKED").length,
    manifestSha256,
    status: statuses.includes("BLOCKED") ? "BLOCKED" : statuses.includes("WARNING") ? "WARNING" : "READY",
    items: statuses.map((status, index) => ({ id: `${packageName}-item-${index}`, name: `Item ${index}`, status })),
  };
}

function makeDetail(batchId: string): ProductionBatchDetail {
  return {
    id: batchId,
    projectId: "project-1",
    name: batchId,
    status: "READY",
    continueOnFailure: false,
    createdAt: "2026-09-09T00:00:00Z",
    updatedAt: "2026-09-09T00:00:00Z",
    total: 1,
    pending: 1,
    running: 0,
    succeeded: 0,
    failed: 0,
    cancelled: 0,
    skipped: 0,
    items: [],
  };
}

function makeBinding(packageKey: string, itemIds: string[], batchId = "batch-a"): ProductionPackageBatchBinding {
  const discoveredPackage = packages.find((item) => item.packageKey === packageKey)!;
  return {
    packageKey,
    packageRoot: discoveredPackage.packageRoot,
    manifestSha256: discoveredPackage.manifestSha256,
    packageName: packageKey,
    batchId,
    chunkIndex: 0,
    chunkCount: 1,
    packageItemIds: itemIds,
    createdAt: "2026-09-09T00:00:00Z",
    sourceKind: "PRODUCTION_PACKAGE",
  };
}

function makeCreateResult(overrides: Partial<ProductionPackageCreateBatchesResult> = {}): ProductionPackageCreateBatchesResult {
  return {
    packageName: "EP01",
    status: "COMPLETE",
    requestedCount: 2,
    createdCount: 2,
    remainingCount: 0,
    remainingItemIds: [],
    batchCount: 1,
    itemCount: 2,
    autoStarted: false,
    batches: [],
    itemMappings: [],
    warnings: [],
    ...overrides,
  };
}

let controller: ReturnType<typeof useShotMultiPackageController> | undefined;

function Harness({ enabled = true }: { enabled?: boolean }) {
  controller = useShotMultiPackageController({
    projectId: "project-1",
    enabled,
    reloadProductionQueues: mocks.reloadProductionQueues,
    onError: mocks.onError,
    onNotice: mocks.onNotice,
  });
  return null;
}

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

async function discover() {
  await act(async () => {
    await controller?.chooseRoot();
  });
}

beforeEach(() => {
  controller = undefined;
  mocks.pickProductionPackageRoot.mockResolvedValue("C:/season");
  mocks.discoverProductionPackages.mockResolvedValue({ rootPath: "C:/season", packages });
  mocks.inspectProductionPackage.mockImplementation(async (_projectId: string, packageRoot: string) => (
    packageRoot.endsWith("EP01")
      ? makeInspection("EP01", "inspection-a", "sha-a", ["READY", "READY"])
      : makeInspection("EP02", "inspection-b", "sha-b", ["READY"])
  ));
  mocks.listProductionPackageBindings.mockResolvedValue([]);
  mocks.getProductionQueue.mockImplementation(async (_projectId: string, batchId: string) => makeDetail(batchId));
  mocks.createProductionPackageBatches.mockResolvedValue(makeCreateResult());
  mocks.reloadProductionQueues.mockResolvedValue(undefined);
  mocks.onError.mockReset();
  mocks.onNotice.mockReset();
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("useShotMultiPackageController", () => {
  it("keeps picker cancel side-effect free", async () => {
    mocks.pickProductionPackageRoot.mockResolvedValueOnce(undefined);
    render(<Harness />);
    await act(async () => { await controller?.chooseRoot(); });

    expect(mocks.discoverProductionPackages).not.toHaveBeenCalled();
    expect(controller?.packages).toEqual([]);
    expect(controller?.rootPath).toBeNull();
  });

  it("discovers and inspects every package, then refreshes", async () => {
    render(<Harness />);
    await discover();

    expect(mocks.discoverProductionPackages).toHaveBeenCalledWith("C:/season");
    expect(mocks.inspectProductionPackage).toHaveBeenCalledTimes(2);
    expect(controller?.inspectProgress).toMatchObject({ current: 2, total: 2 });
    expect(controller?.packages).toHaveLength(2);
    expect(mocks.reloadProductionQueues).toHaveBeenCalled();
  });

  it("keeps a failed package visible while continuing discovery", async () => {
    mocks.inspectProductionPackage.mockImplementation(async (_projectId: string, packageRoot: string) => {
      if (packageRoot.endsWith("EP01")) throw new Error("EP01 invalid");
      return makeInspection("EP02", "inspection-b", "sha-b", ["READY"]);
    });
    render(<Harness />);
    await discover();

    expect(controller?.packages.map((item) => item.packageKey)).toEqual(["package-a", "package-b"]);
    expect(controller?.boardPackages[0]).toMatchObject({ status: "BLOCKED", issueSummary: "操作失败，请查看技术详情。" });
    expect(controller?.boardPackages[1].status).toBe("READY");
  });

  it("coalesces concurrent refreshes into one pending rerun", async () => {
    let resolveBindings!: (value: ProductionPackageBatchBinding[]) => void;
    const firstBindings = new Promise<ProductionPackageBatchBinding[]>((resolve) => { resolveBindings = resolve; });
    mocks.listProductionPackageBindings.mockImplementationOnce(() => firstBindings).mockResolvedValueOnce([]);
    render(<Harness />);

    let firstRefresh!: Promise<void>;
    await act(async () => {
      firstRefresh = controller!.refresh();
      await Promise.resolve();
      await controller!.refresh();
    });
    expect(mocks.listProductionPackageBindings).toHaveBeenCalledTimes(1);
    resolveBindings([]);
    await act(async () => {
      await firstRefresh;
      await flush();
    });

    expect(mocks.listProductionPackageBindings).toHaveBeenCalledTimes(2);
  });

  it("ignores an in-flight discovery after unmount", async () => {
    let resolveDiscovery!: (value: { rootPath: string; packages: ProductionPackageDiscoveryPackage[] }) => void;
    const pendingDiscovery = new Promise<{ rootPath: string; packages: ProductionPackageDiscoveryPackage[] }>((resolve) => {
      resolveDiscovery = resolve;
    });
    mocks.discoverProductionPackages.mockReturnValueOnce(pendingDiscovery);
    const view = render(<Harness />);
    let discovery!: Promise<void>;
    await act(async () => {
      discovery = controller!.chooseRoot();
      await flush();
    });
    expect(mocks.discoverProductionPackages).toHaveBeenCalledTimes(1);
    view.unmount();
    resolveDiscovery({ rootPath: "C:/season", packages });
    await act(async () => { await discovery; });

    expect(mocks.reloadProductionQueues).not.toHaveBeenCalled();
  });

  it("preserves successful details when one batch detail fails", async () => {
    const bindings = [makeBinding("package-a", ["EP01-item-0"], "batch-a"), makeBinding("package-b", ["EP02-item-0"], "batch-b")];
    mocks.listProductionPackageBindings.mockResolvedValue(bindings);
    mocks.getProductionQueue.mockImplementation(async (_projectId: string, batchId: string) => {
      if (batchId === "batch-b") throw new Error("batch-b unavailable");
      return makeDetail(batchId);
    });
    render(<Harness />);
    await act(async () => { await controller?.refresh(); });

    expect(controller?.bindings).toEqual(bindings);
    expect(controller?.batchDetails).toHaveProperty("batch-a");
    expect(controller?.batchDetails).not.toHaveProperty("batch-b");
    expect(mocks.onError).toHaveBeenCalledWith(expect.stringContaining("多生产包看板刷新失败"));
  });

  it("re-inspects before creation and excludes already bound items", async () => {
    mocks.listProductionPackageBindings.mockResolvedValue([makeBinding("package-a", ["EP01-item-0"]) ]);
    render(<Harness />);
    await discover();
    await act(async () => { await controller?.createSelected(["package-a"]); });

    expect(mocks.inspectProductionPackage).toHaveBeenCalledTimes(3);
    expect(mocks.createProductionPackageBatches).toHaveBeenCalledWith("inspection-a", ["EP01-item-1"]);
  });

  it("blocks manifest changes and warning inspections before create", async () => {
    render(<Harness />);
    await discover();
    mocks.inspectProductionPackage.mockResolvedValueOnce(makeInspection("EP01", "inspection-new", "sha-changed", ["READY", "READY"]));
    await act(async () => { await expect(controller!.createSelected(["package-a"])).rejects.toThrow("production-package.json"); });
    expect(mocks.createProductionPackageBatches).not.toHaveBeenCalled();

    mocks.inspectProductionPackage.mockResolvedValueOnce(makeInspection("EP01", "inspection-warning", "sha-a", ["READY", "WARNING"]));
    await act(async () => { await expect(controller!.createSelected(["package-a"])).rejects.toThrow("警告"); });
    expect(mocks.createProductionPackageBatches).not.toHaveBeenCalled();
  });

  it("stops later packages after PARTIAL or CREATE_FAILED", async () => {
    render(<Harness />);
    await discover();
    mocks.createProductionPackageBatches.mockResolvedValueOnce(makeCreateResult({
      status: "PARTIAL",
      createdCount: 1,
      remainingCount: 1,
      remainingItemIds: ["EP01-item-1"],
    }));
    await act(async () => { await controller?.createSelected(["package-a", "package-b"]); });

    expect(mocks.createProductionPackageBatches).toHaveBeenCalledTimes(1);
    expect(controller?.boardPackages[1]).toMatchObject({
      status: "NOT_CREATED",
      issueSummary: expect.stringContaining("部分创建"),
    });
  });

  it("stops later packages after a create failure and never starts a queue", async () => {
    render(<Harness />);
    await discover();
    mocks.createProductionPackageBatches.mockRejectedValueOnce(new Error("EP01 create failed"));
    await act(async () => { await expect(controller!.createSelected(["package-a", "package-b"])).rejects.toThrow("EP01 create failed"); });

    expect(mocks.createProductionPackageBatches).toHaveBeenCalledTimes(1);
    expect(controller?.boardPackages[1]).toMatchObject({
      status: "NOT_CREATED",
      issueSummary: expect.stringContaining("创建失败"),
    });
  });
});
