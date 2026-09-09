import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  createProductionPackageBatches,
  discoverProductionPackages,
  getProductionQueue,
  inspectProductionPackage,
  listProductionPackageBindings,
  pickProductionPackageRoot,
} from "../../../services/tauriClient";
import type { ProductionBatchDetail } from "../../../types/productionQueue";
import type {
  ProductionPackageBatchBinding,
  ProductionPackageDiscoveryPackage,
  ProductionPackageInspectionResult,
} from "../../../types/productionPackage";
import { toUserMessage } from "../../../i18n/errorMessages";
import type {
  MultiPackageBoardInspectProgress,
  MultiPackageBoardPackage,
} from "../../production/MultiPackageProductionBoard";
import {
  buildMultiPackageBoardPackage,
  multiPackageBatchOpenPriority,
  multiPackageInspectionSafetyError,
} from "../shotMultiPackageModel";

export interface UseShotMultiPackageControllerOptions {
  projectId: string;
  enabled: boolean;
  reloadProductionQueues: () => void | Promise<unknown>;
  onError: (message: string | undefined) => void;
  onNotice: (message: string) => void;
}

export function useShotMultiPackageController({
  projectId,
  enabled,
  reloadProductionQueues,
  onError,
  onNotice,
}: UseShotMultiPackageControllerOptions) {
  const [rootPath, setRootPath] = useState<string | null>(null);
  const [packages, setPackages] = useState<ProductionPackageDiscoveryPackage[]>([]);
  const [inspections, setInspections] = useState<Record<string, ProductionPackageInspectionResult>>({});
  const [inspectionErrors, setInspectionErrors] = useState<Record<string, string>>({});
  const [createMessages, setCreateMessages] = useState<Record<string, { status: "CREATE_FAILED" | "NOT_CREATED"; message: string }>>({});
  const [bindings, setBindings] = useState<ProductionPackageBatchBinding[]>([]);
  const [batchDetails, setBatchDetails] = useState<Record<string, ProductionBatchDetail>>({});
  const [isDiscovering, setIsDiscovering] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [inspectProgress, setInspectProgress] = useState<MultiPackageBoardInspectProgress>();
  const multiPackageRunId = useRef(0);
  const multiPackageRefreshInFlight = useRef(false);
  const multiPackageRefreshPending = useRef(false);
  const multiPackageMounted = useRef(true);

  useEffect(() => {
    multiPackageMounted.current = true;
    return () => {
      multiPackageMounted.current = false;
      multiPackageRunId.current += 1;
      multiPackageRefreshPending.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    if (!enabled || !multiPackageMounted.current) return;
    if (multiPackageRefreshInFlight.current) {
      multiPackageRefreshPending.current = true;
      return;
    }
    multiPackageRefreshInFlight.current = true;
    try {
      const nextBindings = await listProductionPackageBindings(projectId);
      if (!multiPackageMounted.current) return;
      const nextDetails: Record<string, ProductionBatchDetail> = {};
      const batchIds = [...new Set(nextBindings.map((binding) => binding.batchId))];
      let detailError: unknown;
      for (const batchId of batchIds) {
        if (!multiPackageMounted.current) return;
        try {
          nextDetails[batchId] = await getProductionQueue(projectId, batchId);
          if (!multiPackageMounted.current) return;
        } catch (error: unknown) {
          detailError = error;
        }
      }
      if (!multiPackageMounted.current) return;
      setBindings(nextBindings);
      setBatchDetails(nextDetails);
      await reloadProductionQueues();
      if (detailError) onError(`多生产包看板刷新失败：${toUserMessage(detailError)}`);
    } catch (error: unknown) {
      if (multiPackageMounted.current) onError(`多生产包看板刷新失败：${toUserMessage(error)}`);
    } finally {
      multiPackageRefreshInFlight.current = false;
      if (multiPackageMounted.current && multiPackageRefreshPending.current) {
        multiPackageRefreshPending.current = false;
        void refresh();
      }
    }
  }, [enabled, onError, projectId, reloadProductionQueues]);

  const chooseRoot = useCallback(async () => {
    if (!enabled || isDiscovering || isCreating) return;
    let runId: number | undefined;
    try {
      const pickedRoot = await pickProductionPackageRoot();
      if (!pickedRoot || !multiPackageMounted.current) return;
      runId = ++multiPackageRunId.current;
      setRootPath(pickedRoot);
      setPackages([]);
      setInspections({});
      setInspectionErrors({});
      setCreateMessages({});
      setInspectProgress({ current: 0, total: 0 });
      setIsDiscovering(true);
      onError(undefined);
      const discovery = await discoverProductionPackages(pickedRoot);
      if (!multiPackageMounted.current || runId !== multiPackageRunId.current) return;
      setRootPath(discovery.rootPath);
      setInspectProgress({ current: 0, total: discovery.packages.length });
      let readyCount = 0;
      let warningCount = 0;
      let blockedCount = 0;
      for (const [index, discoveredPackage] of discovery.packages.entries()) {
        if (!multiPackageMounted.current || runId !== multiPackageRunId.current) return;
        setInspectProgress({
          current: index,
          total: discovery.packages.length,
          currentPackage: discoveredPackage.relativePath || discoveredPackage.packageRoot,
          readyCount,
          warningCount,
          blockedCount,
        });
        const packageKey = discoveredPackage.packageKey;
        try {
          const inspection = await inspectProductionPackage(projectId, discoveredPackage.packageRoot);
          if (!multiPackageMounted.current || runId !== multiPackageRunId.current) return;
          if (inspection.manifestSha256 !== discoveredPackage.manifestSha256) {
            throw new Error("production-package.json 在发现后发生变化，请重新选择根目录。");
          }
          setInspections((current) => ({ ...current, [packageKey]: inspection }));
          setPackages((current) => [...current, discoveredPackage]);
          readyCount += inspection.readyCount;
          warningCount += inspection.warningCount;
          blockedCount += inspection.blockedCount;
        } catch (inspectionError: unknown) {
          if (!multiPackageMounted.current || runId !== multiPackageRunId.current) return;
          setInspectionErrors((current) => ({
            ...current,
            [packageKey]: toUserMessage(inspectionError),
          }));
          setPackages((current) => [...current, discoveredPackage]);
          blockedCount += 1;
        }
        if (!multiPackageMounted.current || runId !== multiPackageRunId.current) return;
        setInspectProgress({
          current: index + 1,
          total: discovery.packages.length,
          readyCount,
          warningCount,
          blockedCount,
        });
      }
      if (multiPackageMounted.current && runId === multiPackageRunId.current) await refresh();
    } catch (discoveryError: unknown) {
      if (multiPackageMounted.current && (runId === undefined || runId === multiPackageRunId.current)) {
        setPackages([]);
        setInspectProgress(undefined);
        onError(`发现生产包失败：${toUserMessage(discoveryError)}`);
      }
    } finally {
      if (multiPackageMounted.current && runId !== undefined && runId === multiPackageRunId.current) {
        setIsDiscovering(false);
        setInspectProgress((current) => current ? { ...current, currentPackage: undefined } : current);
      }
    }
  }, [discoverProductionPackages, enabled, inspectProductionPackage, isCreating, isDiscovering, onError, projectId, refresh]);

  const createSelected = useCallback(async (packageKeys: string[]) => {
    if (!enabled || isCreating || !multiPackageMounted.current) return;
    setIsCreating(true);
    onError(undefined);
    try {
      for (let index = 0; index < packageKeys.length; index += 1) {
        if (!multiPackageMounted.current) return;
        const packageKey = packageKeys[index];
        const discoveredPackage = packages.find((item) => item.packageKey === packageKey);
        try {
          if (!discoveredPackage) {
            throw new Error("该生产包尚未完成检查，请先重新检查。");
          }
          const inspection = await inspectProductionPackage(projectId, discoveredPackage.packageRoot);
          if (!multiPackageMounted.current) return;
          if (inspection.manifestSha256 !== discoveredPackage.manifestSha256) {
            throw new Error("production-package.json 在发现后发生变化，请重新选择根目录。");
          }
          setInspections((current) => ({ ...current, [packageKey]: inspection }));
          const safetyError = multiPackageInspectionSafetyError(inspection);
          if (safetyError) throw new Error(safetyError);
          setCreateMessages((current) => {
            const next = { ...current };
            delete next[packageKey];
            return next;
          });
          const boundItemIds = new Set(
            bindings
              .filter((binding) => binding.packageKey === discoveredPackage.packageKey)
              .flatMap((binding) => binding.packageItemIds),
          );
          const selectedItemIds = inspection.items
            .filter((item) => item.status === "READY" && !boundItemIds.has(item.id))
            .map((item) => item.id);
          if (!selectedItemIds.length) continue;
          const result = await createProductionPackageBatches(inspection.inspectionId, selectedItemIds);
          if (!multiPackageMounted.current) return;
          await refresh();
          if (!multiPackageMounted.current) return;
          if (result.status === "PARTIAL" || result.remainingCount > 0) {
            onNotice(`「${inspection.packageName}」已部分创建；请从剩余项目继续。`);
            setCreateMessages((current) => {
              const next = { ...current };
              for (const deferredKey of packageKeys.slice(index + 1)) {
                next[deferredKey] = {
                  status: "NOT_CREATED",
                  message: "未执行：前一个生产包仅部分创建；请先处理剩余项后再继续。",
                };
              }
              return next;
            });
            break;
          }
        } catch (packageError: unknown) {
          if (!multiPackageMounted.current) return;
          const message = packageError instanceof Error ? packageError.message : toUserMessage(packageError);
          setCreateMessages((current) => {
            const next = {
              ...current,
              [packageKey]: { status: "CREATE_FAILED" as const, message },
            };
            for (const deferredKey of packageKeys.slice(index + 1)) {
              next[deferredKey] = {
                status: "NOT_CREATED",
                message: "未执行：前一个生产包创建失败；可继续创建未创建或剩余项。",
              };
            }
            return next;
          });
          throw new Error(`「${discoveredPackage?.packageRoot ?? packageKey}」创建失败：${message}`);
        }
      }
    } finally {
      if (multiPackageMounted.current) {
        setIsCreating(false);
        await refresh();
      }
    }
  }, [bindings, enabled, isCreating, onError, onNotice, packages, projectId, refresh]);

  const reinspect = useCallback(async (packageKey: string) => {
    if (!enabled || isDiscovering || isCreating || !multiPackageMounted.current) return;
    const discoveredPackage = packages.find((item) => item.packageKey === packageKey);
    if (!discoveredPackage) {
      onError("该生产包尚未完成发现，请重新选择根目录。");
      return;
    }
    onError(undefined);
    try {
      const inspection = await inspectProductionPackage(projectId, discoveredPackage.packageRoot);
      if (!multiPackageMounted.current) return;
      if (inspection.manifestSha256 !== discoveredPackage.manifestSha256) {
        throw new Error("production-package.json 在发现后发生变化，请重新选择根目录。");
      }
      setInspections((current) => ({ ...current, [packageKey]: inspection }));
      setInspectionErrors((current) => {
        const next = { ...current };
        delete next[packageKey];
        return next;
      });
      setCreateMessages((current) => {
        const next = { ...current };
        delete next[packageKey];
        return next;
      });
      await refresh();
    } catch (inspectionError: unknown) {
      if (!multiPackageMounted.current) return;
      const message = toUserMessage(inspectionError);
      setInspectionErrors((current) => ({ ...current, [packageKey]: message }));
      onError("重新检查生产包失败：" + message);
    }
  }, [enabled, inspectProductionPackage, isCreating, isDiscovering, onError, packages, projectId, refresh]);

  const boardPackages = useMemo<MultiPackageBoardPackage[]>(
    () => packages.map((discoveredPackage) => {
      const packageKey = discoveredPackage.packageKey;
      return buildMultiPackageBoardPackage({
        discoveredPackage,
        inspection: inspections[packageKey],
        inspectionError: inspectionErrors[packageKey],
        bindings,
        batchDetails,
        createMessage: createMessages[packageKey],
      });
    }),
    [batchDetails, bindings, createMessages, inspectionErrors, inspections, packages],
  );

  const findDiscoveredPackage = useCallback(
    (packageKey: string) => packages.find((item) => item.packageKey === packageKey),
    [packages],
  );
  const bestBatchIdForPackage = useCallback((packageKey: string, batchIds?: readonly string[]) => {
    const candidates = batchIds ?? [...new Set(
      bindings.filter((binding) => binding.packageKey === packageKey).map((binding) => binding.batchId),
    )];
    return candidates.reduce<string | undefined>((selected, candidate) => {
      if (!selected) return candidate;
      return multiPackageBatchOpenPriority(batchDetails[candidate])
        < multiPackageBatchOpenPriority(batchDetails[selected])
        ? candidate
        : selected;
    }, undefined);
  }, [batchDetails, bindings]);

  return {
    rootPath,
    packages,
    boardPackages,
    isDiscovering,
    isCreating,
    inspectProgress,
    bindings,
    batchDetails,
    refresh,
    chooseRoot,
    createSelected,
    reinspect,
    findDiscoveredPackage,
    bestBatchIdForPackage,
  };
}
