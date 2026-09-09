import { describe, expect, it } from "vitest";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import type {
  ProductionPackageBatchBinding,
  ProductionPackageDiscoveryPackage,
  ProductionPackageInspectionResult,
} from "../../types/productionPackage";
import {
  buildMultiPackageBoardPackage,
  multiPackageBatchOpenPriority,
  multiPackageInspectionSafetyError,
} from "./shotMultiPackageModel";

const discoveredPackage: ProductionPackageDiscoveryPackage = {
  packageKey: "package-a",
  packageRoot: "C:/season/EP01",
  relativePath: "EP01",
  manifestPath: "C:/season/EP01/production-package.json",
  manifestSha256: "sha-a",
};

function inspection(statuses: Array<"READY" | "WARNING" | "BLOCKED">): ProductionPackageInspectionResult {
  return {
    inspectionId: "inspection-a",
    packageName: "EP01",
    itemCount: statuses.length,
    readyCount: statuses.filter((status) => status === "READY").length,
    warningCount: statuses.filter((status) => status === "WARNING").length,
    blockedCount: statuses.filter((status) => status === "BLOCKED").length,
    manifestSha256: "sha-a",
    status: statuses.includes("BLOCKED") ? "BLOCKED" : statuses.includes("WARNING") ? "WARNING" : "READY",
    items: statuses.map((status, index) => ({ id: `item-${index}`, name: `Item ${index}`, status })),
  };
}

function detail(status: ProductionBatchDetail["status"], counts: Partial<Pick<ProductionBatchDetail, "pending" | "running" | "succeeded" | "failed">> = {}): ProductionBatchDetail {
  return {
    id: "batch-a",
    projectId: "project-1",
    name: "EP01",
    status,
    continueOnFailure: false,
    createdAt: "2026-09-09T00:00:00Z",
    updatedAt: "2026-09-09T00:00:00Z",
    total: 2,
    pending: counts.pending ?? 0,
    running: counts.running ?? 0,
    succeeded: counts.succeeded ?? 0,
    failed: counts.failed ?? 0,
    cancelled: 0,
    skipped: 0,
    items: [],
  };
}

function binding(itemIds: string[], batchId = "batch-a"): ProductionPackageBatchBinding {
  return {
    packageKey: "package-a",
    packageRoot: discoveredPackage.packageRoot,
    manifestSha256: discoveredPackage.manifestSha256,
    packageName: "EP01",
    batchId,
    chunkIndex: 0,
    chunkCount: 1,
    packageItemIds: itemIds,
    createdAt: "2026-09-09T00:00:00Z",
    sourceKind: "PRODUCTION_PACKAGE",
  };
}

describe("shotMultiPackageModel", () => {
  it("projects READY packages as creatable", () => {
    const result = buildMultiPackageBoardPackage({
      discoveredPackage,
      inspection: inspection(["READY", "READY"]),
      bindings: [],
      batchDetails: {},
    });

    expect(result.status).toBe("READY");
    expect(result.canCreate).toBe(true);
    expect(result.remainingReadyCount).toBe(2);
  });

  it("projects WARNING and BLOCKED packages safely", () => {
    const warning = buildMultiPackageBoardPackage({
      discoveredPackage,
      inspection: inspection(["READY", "WARNING"]),
      bindings: [],
      batchDetails: {},
    });
    const blocked = buildMultiPackageBoardPackage({
      discoveredPackage,
      inspection: inspection(["READY", "BLOCKED"]),
      bindings: [],
      batchDetails: {},
    });

    expect(warning.status).toBe("WARNING");
    expect(warning.canCreate).toBe(false);
    expect(blocked.status).toBe("BLOCKED");
    expect(blocked.canCreate).toBe(false);
    expect(multiPackageInspectionSafetyError(warningInspection())).toContain("警告");
    expect(multiPackageInspectionSafetyError(inspection(["READY", "BLOCKED"]))).toContain("阻塞");
  });

  it("aggregates bindings and batch details into PARTIAL projection", () => {
    const result = buildMultiPackageBoardPackage({
      discoveredPackage,
      inspection: inspection(["READY", "READY", "READY"]),
      bindings: [binding(["item-0"])],
      batchDetails: { "batch-a": detail("RUNNING", { running: 1 }) },
    });

    expect(result.status).toBe("RUNNING");
    expect(result.boundItemCount).toBe(1);
    expect(result.remainingCount).toBe(2);
    expect(result.remainingReadyCount).toBe(2);
    expect(result.batchIds).toEqual(["batch-a"]);
    expect(result.running).toBe(1);
    expect(result.pending).toBe(2);
  });

  it("projects a completed PARTIAL package and preserves remaining counts", () => {
    const result = buildMultiPackageBoardPackage({
      discoveredPackage,
      inspection: inspection(["READY", "READY", "READY"]),
      bindings: [binding(["item-0"])],
      batchDetails: { "batch-a": detail("COMPLETED", { succeeded: 1 }) },
    });

    expect(result.status).toBe("PARTIAL");
    expect(result.remainingCount).toBe(2);
    expect(result.canCreate).toBe(true);
  });

  it("orders batches by running, failed, ready, then terminal priority", () => {
    expect(multiPackageBatchOpenPriority(detail("RUNNING", { running: 1 }))).toBe(0);
    expect(multiPackageBatchOpenPriority(detail("COMPLETED", { failed: 1 }))).toBe(1);
    expect(multiPackageBatchOpenPriority(detail("READY", { pending: 1 }))).toBe(2);
    expect(multiPackageBatchOpenPriority()).toBe(3);
  });
});

function warningInspection(): ProductionPackageInspectionResult {
  return inspection(["READY", "WARNING"]);
}
