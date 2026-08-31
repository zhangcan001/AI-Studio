import { useEffect, useMemo, useRef, useState } from "react";
import "./MultiPackageProductionBoard.css";

export type MultiPackageBoardPackageStatus =
  | "READY"
  | "WARNING"
  | "BLOCKED"
  | "NOT_CREATED"
  | "UPDATED"
  | "CREATING"
  | "CREATE_FAILED"
  | "CREATED"
  | "RUNNING"
  | "COMPLETED"
  | "COMPLETED_WITH_FAILURE";

export interface MultiPackageBoardPackage {
  packageKey: string;
  packageRoot: string;
  relativePath: string;
  packageName: string;
  itemCount: number;
  status: MultiPackageBoardPackageStatus;
  readyCount?: number;
  warningCount?: number;
  blockedCount?: number;
  batchIds?: string[];
  pending?: number;
  running?: number;
  succeeded?: number;
  failed?: number;
  firstError?: string;
  issueSummary?: string;
}

export interface MultiPackageBoardInspectProgress {
  current: number;
  total: number;
  currentPackage?: string;
  readyCount?: number;
  warningCount?: number;
  blockedCount?: number;
}

export interface MultiPackageProductionBoardProps {
  packages?: MultiPackageBoardPackage[];
  rootPath?: string | null;
  isDiscovering?: boolean;
  inspectProgress?: MultiPackageBoardInspectProgress;
  isCreating?: boolean;
  selectedPackageKeys?: string[];
  onChooseRoot?: () => void | Promise<void>;
  onOpenPackage?: (packageKey: string) => void;
  onHandleWarning?: (packageKey: string) => void;
  onViewIssues?: (packageKey: string) => void;
  onReinspect?: (packageKey: string) => void;
  onOpenBatch?: (packageKey: string, batchIds: string[]) => void;
  onCreateSelected?: (keys: string[]) => void | Promise<void>;
  onRefresh?: () => void | Promise<void>;
  refreshIntervalMs?: number;
  pollingEnabled?: boolean;
}

type BoardFilter = "all" | "not-created" | "issues" | "created" | "running" | "completed";

const MAX_PACKAGES = 100;
const MAX_SELECTED_ITEMS = 10_000;
const FILTERS: Array<{ value: BoardFilter; label: string }> = [
  { value: "all", label: "全部" },
  { value: "not-created", label: "未创建" },
  { value: "issues", label: "问题" },
  { value: "created", label: "已创建" },
  { value: "running", label: "运行中" },
  { value: "completed", label: "已完成" },
];

const STATUS_LABELS: Record<MultiPackageBoardPackageStatus, string> = {
  READY: "可创建",
  WARNING: "有警告",
  BLOCKED: "已阻塞",
  NOT_CREATED: "未创建",
  UPDATED: "已更新",
  CREATING: "创建中",
  CREATE_FAILED: "创建失败",
  CREATED: "已创建",
  RUNNING: "运行中",
  COMPLETED: "已完成",
  COMPLETED_WITH_FAILURE: "完成但有失败",
};

export function MultiPackageProductionBoard({
  packages = [],
  rootPath,
  isDiscovering = false,
  inspectProgress,
  isCreating = false,
  selectedPackageKeys,
  onChooseRoot,
  onOpenPackage,
  onHandleWarning,
  onViewIssues,
  onReinspect,
  onOpenBatch,
  onCreateSelected,
  onRefresh,
  refreshIntervalMs = 5000,
  pollingEnabled = true,
}: MultiPackageProductionBoardProps) {
  const [filter, setFilter] = useState<BoardFilter>("all");
  const [localSelection, setLocalSelection] = useState<Set<string>>(() => new Set(
    selectedPackageKeys ?? packages.filter((item) => item.status === "READY").map((item) => item.packageKey),
  ));
  const [operationStatuses, setOperationStatuses] = useState<Record<string, { status: MultiPackageBoardPackageStatus; observedStatus: MultiPackageBoardPackageStatus }>>({});
  const [createError, setCreateError] = useState<string>();
  const [isCreatingLocally, setIsCreatingLocally] = useState(false);
  const lastExternalSelectionRef = useRef(selectedPackageKeys === undefined ? "__unset__" : selectedPackageKeys.join("\u0000"));
  const lastPackageSignatureRef = useRef(packages.map((item) => `${item.packageKey}:${item.status}`).join("\u0000"));
  const refreshInFlightRef = useRef(false);

  const externalSelectionSignature = selectedPackageKeys === undefined ? "__unset__" : selectedPackageKeys.join("\u0000");
  useEffect(() => {
    if (selectedPackageKeys === undefined || externalSelectionSignature === lastExternalSelectionRef.current) return;
    lastExternalSelectionRef.current = externalSelectionSignature;
    setLocalSelection(new Set(selectedPackageKeys));
  }, [externalSelectionSignature, selectedPackageKeys]);

  const packageSignature = packages.map((item) => `${item.packageKey}:${item.status}`).join("\u0000");
  useEffect(() => {
    if (selectedPackageKeys !== undefined || packageSignature === lastPackageSignatureRef.current) return;
    lastPackageSignatureRef.current = packageSignature;
    setLocalSelection((previous) => {
      const packageKeys = new Set(packages.map((item) => item.packageKey));
      const next = new Set([...previous].filter((key) => packageKeys.has(key)));
      packages.forEach((item) => {
        if (item.status === "READY" && !previous.has(item.packageKey)) next.add(item.packageKey);
      });
      return next;
    });
  }, [packageSignature, packages, selectedPackageKeys]);

  useEffect(() => {
    if (!pollingEnabled || !onRefresh || isDiscovering || refreshIntervalMs <= 0) return undefined;
    const timer = window.setInterval(() => {
      if (document.visibilityState === "hidden" || refreshInFlightRef.current) return;
      refreshInFlightRef.current = true;
      void Promise.resolve(onRefresh()).finally(() => {
        refreshInFlightRef.current = false;
      });
    }, refreshIntervalMs);
    return () => window.clearInterval(timer);
  }, [isDiscovering, onRefresh, pollingEnabled, refreshIntervalMs]);

  const effectiveStatus = (item: MultiPackageBoardPackage): MultiPackageBoardPackageStatus => {
    const local = operationStatuses[item.packageKey];
    return local && local.observedStatus === item.status ? local.status : item.status;
  };

  const visiblePackages = useMemo(
    () => packages.filter((item) => matchesFilter(effectiveStatus(item), filter)),
    [filter, operationStatuses, packages],
  );
  const selectablePackages = useMemo(
    () => packages.filter((item) => isSelectableStatus(effectiveStatus(item))),
    [operationStatuses, packages],
  );
  const selectedPackages = useMemo(
    () => packages.filter((item) => localSelection.has(item.packageKey) && isSelectableStatus(effectiveStatus(item))),
    [localSelection, operationStatuses, packages],
  );
  const selectedItemCount = selectedPackages.reduce((sum, item) => sum + Math.max(0, item.itemCount), 0);
  const isOverPackageLimit = packages.length > MAX_PACKAGES || selectedPackages.length > MAX_PACKAGES;
  const isOverItemLimit = selectedItemCount > MAX_SELECTED_ITEMS;
  const isBusy = isCreating || isCreatingLocally || isDiscovering;
  const canCreate = Boolean(onCreateSelected) && selectedPackages.length > 0 && !isBusy && !isOverPackageLimit && !isOverItemLimit;
  const summary = summarizePackages(packages, effectiveStatus);

  function setPackageSelected(packageKey: string, checked: boolean) {
    setLocalSelection((previous) => {
      const next = new Set(previous);
      if (checked) next.add(packageKey);
      else next.delete(packageKey);
      return next;
    });
    setCreateError(undefined);
  }

  async function createSelected() {
    if (!canCreate || isCreatingLocally) return;
    const keys = packages.filter((item) => localSelection.has(item.packageKey) && isSelectableStatus(effectiveStatus(item))).map((item) => item.packageKey);
    if (!keys.length) return;
    setIsCreatingLocally(true);
    setCreateError(undefined);
    setOperationStatuses((previous) => ({ ...previous, ...Object.fromEntries(keys.map((key) => {
      const item = packages.find((candidate) => candidate.packageKey === key);
      return [key, { status: "CREATING", observedStatus: item?.status ?? "READY" }];
    })) }));
    try {
      await onCreateSelected?.(keys);
      setOperationStatuses((previous) => ({ ...previous, ...Object.fromEntries(keys.map((key) => {
        const item = packages.find((candidate) => candidate.packageKey === key);
        return [key, { status: "CREATED", observedStatus: item?.status ?? "READY" }];
      })) }));
      setLocalSelection((previous) => new Set([...previous].filter((key) => !keys.includes(key))));
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : typeof error === "string" ? error : "创建所选生产批次失败，请检查后从未创建或剩余项继续。";
      setCreateError(message);
      setOperationStatuses((previous) => ({ ...previous, ...Object.fromEntries(keys.map((key) => {
        const item = packages.find((candidate) => candidate.packageKey === key);
        return [key, { status: "CREATE_FAILED", observedStatus: item?.status ?? "READY" }];
      })) }));
    } finally {
      setIsCreatingLocally(false);
    }
  }

  return (
    <section className="multi-package-production-board" aria-label="批量生产包看板" aria-busy={isBusy}>
      <header className="multi-package-production-board-heading">
        <div>
          <span className="multi-package-production-board-eyebrow">整季生产管理</span>
          <h2>批量生产包</h2>
          <p>集中检查整季生产包，按需创建生产批次。不会自动开始生成。</p>
        </div>
        <div className="multi-package-production-board-heading-actions">
          {onRefresh && <button type="button" className="multi-package-production-board-quiet" onClick={() => void onRefresh()} disabled={isBusy}>刷新看板</button>}
          <span className="multi-package-production-board-state">{isDiscovering ? "发现中" : isBusy ? "处理中" : "手动创建"}</span>
        </div>
      </header>

      <div className="multi-package-production-board-root">
        <div>
          <strong>生产包根目录</strong>
          <span title={rootPath ?? ""}>{rootPath || "尚未选择根目录"}</span>
        </div>
        <button type="button" onClick={() => void onChooseRoot?.()} disabled={!onChooseRoot || isBusy}>选择根目录</button>
      </div>

      {(isDiscovering || inspectProgress) && <InspectionProgress isDiscovering={isDiscovering} progress={inspectProgress} />}

      {!packages.length ? (
        <div className="multi-package-production-board-empty" role="status">
          <strong>还没有批量生产包</strong>
          <p>选择根目录后检查整季生产包，确认问题后再创建所选生产批次。</p>
          <span>不会自动开始生成，所有创建动作都需要手动确认。</span>
        </div>
      ) : (
        <>
          <div className="multi-package-production-board-summary" aria-label="批量生产摘要">
            <SummaryCard label="生产包数" value={summary.packageCount} />
            <SummaryCard label="镜头总数" value={summary.itemCount} />
            <SummaryCard label="READY · 可创建" value={summary.readyCount} tone="success" />
            <SummaryCard label="WARNING · 警告" value={summary.warningCount} tone="accent" />
            <SummaryCard label="BLOCKED · 阻塞" value={summary.blockedCount} tone="danger" />
            <SummaryCard label="已创建" value={summary.createdCount} tone="success" />
            <SummaryCard label="运行中" value={summary.runningCount} tone="accent" />
            <SummaryCard label="已完成" value={summary.completedCount} tone="success" />
            <SummaryCard label="失败 / 阻塞" value={summary.failedCount + summary.blockedCount} tone="danger" />
          </div>

          <div className="multi-package-production-board-toolbar">
            <div className="multi-package-production-board-filters" role="group" aria-label="生产包筛选">
              {FILTERS.map((item) => <button key={item.value} type="button" className={filter === item.value ? "is-active" : undefined} onClick={() => setFilter(item.value)} disabled={isBusy}>{item.label}<span>{filterCount(packages, item.value, effectiveStatus)}</span></button>)}
            </div>
            <span className="multi-package-production-board-selection">已选 {selectedPackages.length} 个生产包 · {selectedItemCount} 个镜头（可创建 {selectablePackages.length} 个）</span>
          </div>

          {(isOverPackageLimit || isOverItemLimit || createError) && <div className="multi-package-production-board-alert" role="alert">
            {isOverPackageLimit && <span>生产包数量超过上限：最多 {MAX_PACKAGES} 个，当前发现 {packages.length} 个或已选 {selectedPackages.length} 个。</span>}
            {isOverItemLimit && <span>选中镜头超过上限：最多 {MAX_SELECTED_ITEMS} 个，当前为 {selectedItemCount} 个。</span>}
            {createError && <span>{createError} 失败后可从未创建 / 剩余项继续，不会隐式重试。</span>}
          </div>}

          <div className="multi-package-production-board-table-wrap">
            <table className="multi-package-production-board-table">
              <caption className="sr-only">批量生产包列表</caption>
              <thead><tr><th scope="col">选择</th><th scope="col">生产包</th><th scope="col">镜头</th><th scope="col">状态</th><th scope="col">批次 / 进度</th><th scope="col">问题</th><th scope="col">操作</th></tr></thead>
              <tbody>
                {visiblePackages.map((item) => <PackageRow
                  key={item.packageKey}
                  item={item}
                  status={effectiveStatus(item)}
                  checked={localSelection.has(item.packageKey)}
                  disabled={!isSelectableStatus(effectiveStatus(item)) || isBusy}
                  onCheckedChange={setPackageSelected}
                  onOpenPackage={onOpenPackage}
                  onHandleWarning={onHandleWarning}
                  onViewIssues={onViewIssues}
                  onReinspect={onReinspect}
                  onOpenBatch={onOpenBatch}
                />)}
              </tbody>
            </table>
            {!visiblePackages.length && <p className="multi-package-production-board-filter-empty">当前筛选没有生产包。</p>}
          </div>

          <footer className="multi-package-production-board-footer">
            <span>创建前请确认选中范围；创建后仍需在生产批次中手动执行。</span>
            <button type="button" className="multi-package-production-board-create" onClick={() => void createSelected()} disabled={!canCreate}>
              {isCreatingLocally ? "创建中…" : `创建所选生产批次（${selectedPackages.length} 个生产包 · ${selectedItemCount} 个镜头）`}
            </button>
          </footer>
        </>
      )}
    </section>
  );
}

function PackageRow({
  item,
  status,
  checked,
  disabled,
  onCheckedChange,
  onOpenPackage,
  onHandleWarning,
  onViewIssues,
  onReinspect,
  onOpenBatch,
}: {
  item: MultiPackageBoardPackage;
  status: MultiPackageBoardPackageStatus;
  checked: boolean;
  disabled: boolean;
  onCheckedChange: (key: string, checked: boolean) => void;
  onOpenPackage?: (key: string) => void;
  onHandleWarning?: (key: string) => void;
  onViewIssues?: (key: string) => void;
  onReinspect?: (key: string) => void;
  onOpenBatch?: (key: string, batchIds: string[]) => void;
}) {
  const progressTotal = Math.max(0, item.itemCount);
  const succeeded = Math.max(0, item.succeeded ?? 0);
  const progress = progressTotal ? Math.min(100, Math.round((succeeded / progressTotal) * 100)) : 0;
  const issue = item.firstError || item.issueSummary;
  const canOpenBatch = ["CREATED", "RUNNING", "COMPLETED", "COMPLETED_WITH_FAILURE"].includes(status);

  return (
    <tr data-package-key={item.packageKey}>
      <td className="multi-package-production-board-checkbox-cell"><input type="checkbox" aria-label={`选择生产包 ${item.packageName}`} checked={checked} disabled={disabled} onChange={(event) => onCheckedChange(item.packageKey, event.currentTarget.checked)} /></td>
      <td className="multi-package-production-board-package-cell"><strong>{item.packageName}</strong><code>{item.relativePath || item.packageRoot}</code></td>
      <td>{item.itemCount}</td>
      <td><span className={`multi-package-production-board-status status-${status.toLowerCase()}`}>{STATUS_LABELS[status]}</span></td>
      <td><div className="multi-package-production-board-progress" aria-label={`${progress}%，${succeeded}/${progressTotal}`}><span style={{ width: `${progress}%` }} /><small>{item.batchIds?.length ? `批次 ${item.batchIds.join("、")}` : "尚未创建"} · {item.pending ?? Math.max(0, progressTotal - succeeded)} 待处理 / {item.running ?? 0} 运行中 / {succeeded} 已完成{item.failed ? ` / ${item.failed} 失败` : ""}</small></div></td>
      <td>{issue ? <span className="multi-package-production-board-issue">{issue}</span> : <span className="multi-package-production-board-muted">—</span>}</td>
      <td><div className="multi-package-production-board-actions">
        {canOpenBatch ? <button type="button" onClick={() => onOpenBatch?.(item.packageKey, item.batchIds ?? [])} disabled={!onOpenBatch}>打开生产批次</button> : <button type="button" className="multi-package-production-board-quiet" onClick={() => onOpenPackage?.(item.packageKey)} disabled={!onOpenPackage}>打开生产包</button>}
        {status === "WARNING" && <button type="button" className="multi-package-production-board-quiet" onClick={() => onHandleWarning?.(item.packageKey)} disabled={!onHandleWarning}>在单生产包中处理</button>}
        {status === "BLOCKED" && <><button type="button" className="multi-package-production-board-quiet" onClick={() => onViewIssues?.(item.packageKey)} disabled={!onViewIssues}>查看问题</button><button type="button" className="multi-package-production-board-quiet" onClick={() => onReinspect?.(item.packageKey)} disabled={!onReinspect}>重新检查</button></>}
      </div></td>
    </tr>
  );
}

function InspectionProgress({ isDiscovering, progress }: { isDiscovering: boolean; progress?: MultiPackageBoardInspectProgress }) {
  const current = progress?.current ?? 0;
  const total = progress?.total ?? 0;
  const percent = total ? Math.min(100, Math.round((current / total) * 100)) : 0;
  return <div className="multi-package-production-board-discovery" role="status" aria-live="polite"><strong>{isDiscovering ? "正在发现生产包" : "发现检查进度"}</strong><span>{current} / {total}{progress?.currentPackage ? ` · ${progress.currentPackage}` : ""}</span><div role="progressbar" aria-valuemin={0} aria-valuemax={total || 1} aria-valuenow={current} aria-label="生产包发现进度"><span style={{ width: `${percent}%` }} /></div>{progress && <small>可创建 {progress.readyCount ?? 0} · 警告 {progress.warningCount ?? 0} · 阻塞 {progress.blockedCount ?? 0}</small>}</div>;
}

function SummaryCard({ label, value, tone }: { label: string; value: number; tone?: "success" | "danger" | "accent" }) {
  return <div className={`multi-package-production-board-summary-card${tone ? ` tone-${tone}` : ""}`}><strong>{value}</strong><span>{label}</span></div>;
}

function isSelectableStatus(status: MultiPackageBoardPackageStatus): boolean {
  return ["READY", "WARNING", "NOT_CREATED", "UPDATED", "CREATE_FAILED"].includes(status);
}

function matchesFilter(status: MultiPackageBoardPackageStatus, filter: BoardFilter): boolean {
  if (filter === "all") return true;
  if (filter === "not-created") return ["READY", "WARNING", "BLOCKED", "NOT_CREATED", "UPDATED", "CREATE_FAILED"].includes(status);
  if (filter === "issues") return ["WARNING", "BLOCKED", "CREATE_FAILED", "COMPLETED_WITH_FAILURE"].includes(status);
  if (filter === "created") return ["CREATED", "UPDATED"].includes(status);
  if (filter === "running") return status === "RUNNING" || status === "CREATING";
  return status === "COMPLETED" || status === "COMPLETED_WITH_FAILURE";
}

function filterCount(packages: MultiPackageBoardPackage[], filter: BoardFilter, statusOf: (item: MultiPackageBoardPackage) => MultiPackageBoardPackageStatus): number {
  return packages.filter((item) => matchesFilter(statusOf(item), filter)).length;
}

function summarizePackages(packages: MultiPackageBoardPackage[], statusOf: (item: MultiPackageBoardPackage) => MultiPackageBoardPackageStatus) {
  return packages.reduce((summary, item) => {
    const status = statusOf(item);
    summary.itemCount += item.itemCount;
    if (["CREATED", "RUNNING", "COMPLETED", "COMPLETED_WITH_FAILURE"].includes(status)) summary.createdCount += 1;
    if (status === "RUNNING") summary.runningCount += 1;
    if (["COMPLETED", "COMPLETED_WITH_FAILURE"].includes(status)) summary.completedCount += 1;
    if (status === "READY") summary.readyCount += 1;
    if (status === "WARNING") summary.warningCount += 1;
    if (status === "BLOCKED") summary.blockedCount += 1;
    if (["CREATE_FAILED", "COMPLETED_WITH_FAILURE"].includes(status)) summary.failedCount += 1;
    return summary;
  }, { packageCount: packages.length, itemCount: 0, readyCount: 0, warningCount: 0, blockedCount: 0, createdCount: 0, runningCount: 0, completedCount: 0, failedCount: 0 });
}
