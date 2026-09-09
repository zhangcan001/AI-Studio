import type { ProductionBatchDetail } from "../../types/productionQueue";
import type {
  ProductionPackageBatchBinding,
  ProductionPackageDiscoveryPackage,
  ProductionPackageInspectionResult,
} from "../../types/productionPackage";
import type { MultiPackageBoardPackage } from "../production/MultiPackageProductionBoard";

export function multiPackageBatchOpenPriority(batch?: ProductionBatchDetail): number {
  if (!batch) return 3;
  if (batch.status === "RUNNING" || batch.running > 0) return 0;
  if (batch.failed > 0) return 1;
  if (batch.status === "READY" || batch.status === "PAUSED" || batch.pending > 0) return 2;
  return 3;
}

export function multiPackageIdentity(discoveredPackage: ProductionPackageDiscoveryPackage): string {
  return discoveredPackage.packageKey;
}

export function multiPackageInspectionSafetyError(inspection: ProductionPackageInspectionResult): string | undefined {
  const hasWarning = inspection.status === "WARNING"
    || inspection.warningCount > 0
    || inspection.items.some((item) => item.status === "WARNING")
    || Boolean(inspection.warnings?.length);
  if (hasWarning) return "该生产包包含需要人工确认的警告镜头，请先在单生产包中处理。";

  const hasBlocked = inspection.status === "BLOCKED"
    || inspection.blockedCount > 0
    || inspection.items.some((item) => item.status === "BLOCKED")
    || Boolean(inspection.errors?.length);
  if (hasBlocked) return "该生产包包含阻塞项目，不能批量创建。";

  const hasUnsupportedStatus = inspection.status !== undefined && inspection.status !== "READY";
  const hasUnsupportedItem = inspection.items.some((item) => item.status !== "READY");
  if (hasUnsupportedStatus || hasUnsupportedItem) return "该生产包当前检查状态不是 READY，不能批量创建。";
  return undefined;
}

export function buildMultiPackageBoardPackage(input: {
  discoveredPackage: ProductionPackageDiscoveryPackage;
  inspection?: ProductionPackageInspectionResult;
  inspectionError?: string;
  bindings: ProductionPackageBatchBinding[];
  batchDetails: Record<string, ProductionBatchDetail>;
  createMessage?: { status: "CREATE_FAILED" | "NOT_CREATED"; message: string };
}): MultiPackageBoardPackage {
  const { discoveredPackage, inspection, inspectionError, bindings, batchDetails, createMessage } = input;
  const packageBindings = bindings.filter(
    (binding) => binding.packageKey === discoveredPackage.packageKey,
  );
  const packageBatchIds = [...new Set(packageBindings.map((binding) => binding.batchId))];
  const details = packageBatchIds
    .map((batchId) => batchDetails[batchId])
    .filter((detail): detail is ProductionBatchDetail => Boolean(detail));
  const stats = details.reduce((current, detail) => ({
    pending: current.pending + detail.pending,
    running: current.running + detail.running,
    succeeded: current.succeeded + detail.succeeded,
    failed: current.failed + detail.failed,
  }), { pending: 0, running: 0, succeeded: 0, failed: 0 });
  const currentItems = inspection?.items ?? [];
  const currentItemIds = new Set(currentItems.map((item) => item.id));
  const boundItemIds = new Set(
    packageBindings
      .flatMap((binding) => binding.packageItemIds)
      .filter((itemId) => currentItemIds.has(itemId)),
  );
  const itemCount = inspection?.itemCount ?? currentItems.length;
  const remainingItems = currentItems.filter((item) => !boundItemIds.has(item.id));
  const boundItemCount = boundItemIds.size;
  const remainingCount = remainingItems.length;
  const remainingReadyCount = remainingItems.filter((item) => item.status === "READY").length;
  const remainingWarningCount = remainingItems.filter((item) => item.status === "WARNING").length;
  const remainingBlockedCount = remainingItems.filter((item) => item.status === "BLOCKED").length;
  const hasActiveBatch = details.some((detail) => detail.status === "RUNNING" || detail.running > 0);
  const allBatchDetailsAvailable = details.length === packageBatchIds.length;
  const allBatchesTerminal = packageBatchIds.length > 0 && allBatchDetailsAvailable && details.every((detail) => (
    detail.status === "COMPLETED"
      || detail.succeeded + detail.failed + detail.cancelled + detail.skipped >= detail.total
  ));
  const allItemsBound = itemCount > 0 && currentItems.length === itemCount && remainingCount === 0;
  const hasInspectionWarnings = Boolean(
    inspection?.status === "WARNING"
      || (inspection?.warnings?.length ?? 0) > 0
      || (inspection?.warningCount ?? 0) > 0
      || currentItems.some((item) => item.status === "WARNING"),
  );
  const hasInspectionBlocked = Boolean(
    inspection?.status === "BLOCKED"
      || (inspection?.errors?.length ?? 0) > 0
      || (inspection?.blockedCount ?? 0) > 0
      || currentItems.some((item) => item.status === "BLOCKED"),
  );
  let status: MultiPackageBoardPackage["status"];
  if (createMessage) status = createMessage.status;
  else if (inspectionError || !inspection) status = "BLOCKED";
  else if (!packageBindings.length) status = hasInspectionBlocked
    ? "BLOCKED"
    : hasInspectionWarnings ? "WARNING" : "READY";
  else if (hasActiveBatch) status = "RUNNING";
  else if (allItemsBound && allBatchesTerminal && stats.failed > 0) status = "COMPLETED_WITH_FAILURE";
  else if (allItemsBound && allBatchesTerminal) status = "COMPLETED";
  else if (allItemsBound) status = "CREATED";
  else status = "PARTIAL";

  const canCreate = !createMessage
    ? (status === "READY" || status === "PARTIAL")
      && remainingCount > 0
      && remainingReadyCount === remainingCount
      && remainingWarningCount === 0
      && remainingBlockedCount === 0
    : createMessage.status === "NOT_CREATED"
      && remainingCount > 0
      && remainingReadyCount === remainingCount
      && remainingWarningCount === 0
      && remainingBlockedCount === 0;
  const inspectionWarningCount = inspection?.warningCount ?? 0;
  const inspectionBlockedCount = inspection?.blockedCount ?? 0;

  const issueSummary = createMessage?.message
    ?? inspectionError
    ?? (inspection?.errors?.length ? inspection.errors.map(packageDiagnosticText).join("；") : undefined)
    ?? (inspection && (hasInspectionWarnings || inspectionWarningCount > 0 || inspectionBlockedCount > 0)
      ? `READY ${inspection.readyCount} · WARNING ${inspectionWarningCount} · BLOCKED ${inspectionBlockedCount}`
      : undefined);
  const firstError = createMessage?.message
    ?? details.flatMap((detail) => detail.items)
      .map((item) => item.errorMessage || item.errorCode)
      .find((value): value is string => Boolean(value));
  return {
    packageKey: multiPackageIdentity(discoveredPackage),
    packageRoot: discoveredPackage.packageRoot,
    relativePath: discoveredPackage.relativePath,
    packageName: inspection?.packageName ?? displayMultiPackageName(discoveredPackage),
    itemCount,
    status,
    readyCount: inspection?.readyCount ?? 0,
    warningCount: inspection?.warningCount ?? 0,
    blockedCount: inspection?.blockedCount ?? 0,
    boundItemCount,
    remainingCount,
    remainingReadyCount,
    remainingWarningCount,
    remainingBlockedCount,
    canCreate,
    batchIds: packageBatchIds,
    pending: stats.pending + remainingCount,
    running: stats.running,
    succeeded: stats.succeeded,
    failed: stats.failed,
    firstError,
    issueSummary,
  };
}

function packageDiagnosticText(issue: unknown): string {
  if (typeof issue === "string") return issue;
  if (issue && typeof issue === "object") {
    const value = issue as { code?: unknown; message?: unknown; detail?: unknown };
    return [value.code, value.message, value.detail]
      .filter((item): item is string => typeof item === "string" && item.length > 0)
      .join("：");
  }
  return "未知问题";
}

function displayMultiPackageName(discoveredPackage: ProductionPackageDiscoveryPackage): string {
  const source = discoveredPackage.relativePath || discoveredPackage.packageRoot;
  return source.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || source;
}
