import { useEffect, useMemo, useState } from "react";
import "./ProductionMonitor.css";

export const PRODUCTION_MONITOR_PAGE_SIZE = 50;

export type ProductionMonitorFilter = "ALL" | "RUNNING" | "FAILED" | "COMPLETED";

/**
 * Deliberately permissive read-model types. The Host owns API adaptation and
 * may pass additional fields from a newer backend without changing this UI.
 */
export interface ProductionMonitorItemReadModel {
  id?: string | number | null;
  itemId?: string | number | null;
  ordinal?: number | null;
  status?: string | null;
  productionItemStatus?: string | null;
  taskStatus?: string | null;
  name?: string | null;
  title?: string | null;
  label?: string | null;
  shotName?: string | null;
  sceneName?: string | null;
  promptText?: string | null;
  errorCode?: string | null;
  errorMessage?: string | null;
  error?: string | { message?: string | null; detail?: string | null } | null;
  message?: string | null;
  failureReason?: string | null;
  errorDetail?: string | null;
  reason?: string | null;
  videoUrl?: string | null;
  assetType?: string | null;
  mimeType?: string | null;
  mediaType?: string | null;
  mediaUrl?: string | null;
  assetUrl?: string | null;
  videoPath?: string | null;
  outputUrl?: string | null;
  outputPath?: string | null;
  fileUrl?: string | null;
  filePath?: string | null;
  outputLocation?: string | null;
  fileLocation?: string | null;
  localPath?: string | null;
  assetId?: string | null;
  selectedAssetId?: string | null;
  outputAssets?: readonly unknown[] | null;
  recordAvailable?: boolean | null;
  productRecordAvailable?: boolean | null;
  output?: unknown;
  product?: unknown;
  asset?: unknown;
  media?: unknown;
}

export interface ProductionMonitorSummaryReadModel {
  total?: number | null;
  pending?: number | null;
  running?: number | null;
  succeeded?: number | null;
  failed?: number | null;
  cancelled?: number | null;
  skipped?: number | null;
  totalItems?: number | null;
  pendingItems?: number | null;
  runningItems?: number | null;
  succeededItems?: number | null;
  failedItems?: number | null;
  cancelledItems?: number | null;
  skippedItems?: number | null;
}

export interface ProductionMonitorBatchReadModel {
  id?: string | number | null;
  batchId?: string | number | null;
  name?: string | null;
  batchName?: string | null;
  status?: string | null;
  batch?: ProductionMonitorBatchReadModel | null;
  total?: number | null;
  pending?: number | null;
  running?: number | null;
  succeeded?: number | null;
  failed?: number | null;
  cancelled?: number | null;
  skipped?: number | null;
  successCount?: number | null;
  totalItems?: number | null;
  pendingItems?: number | null;
  runningItems?: number | null;
  succeededItems?: number | null;
  failedItems?: number | null;
  cancelledItems?: number | null;
  skippedItems?: number | null;
  items?: readonly ProductionMonitorItemReadModel[] | null;
  summary?: ProductionMonitorSummaryReadModel | null;
  results?: readonly ProductionMonitorItemReadModel[] | null;
  entries?: readonly ProductionMonitorItemReadModel[] | null;
}

export interface ProductionMonitorProps {
  /** A complete batch snapshot. It is rendered as-is until the Host supplies a new snapshot. */
  batch?: ProductionMonitorBatchReadModel | null;
  /** Alias useful when the Host calls this value a read model. */
  readModel?: ProductionMonitorBatchReadModel | null;
  onRetry?: (itemId: string) => void | Promise<void>;
  onRetryItem?: (itemId: string) => void | Promise<void>;
  /** Ask the Host to play a known output; this component never loads media itself. */
  onPlay?: (itemId: string, assetId: string) => void | Promise<void>;
  onOpenFileLocation?: (itemId: string, filePath?: string) => void | Promise<void>;
  onOpenFile?: (itemId: string, filePath?: string) => void | Promise<void>;
  onViewAllProducts?: () => void | Promise<void>;
  onViewAllFinishedProducts?: () => void | Promise<void>;
  onViewAllOutputs?: () => void | Promise<void>;
  onOpenProductsFolder?: () => void | Promise<void>;
  onOpenOutputFolder?: () => void | Promise<void>;
  onOpenFinishedProductsFolder?: () => void | Promise<void>;
  onExportProductList?: () => void | Promise<void>;
  onExportManifest?: () => void | Promise<void>;
  onExportProducts?: () => void | Promise<void>;
  onSelectNextProductionPackage?: () => void | Promise<void>;
  onSelectNextPackage?: () => void | Promise<void>;
  onChooseNextProductionPackage?: () => void | Promise<void>;
}

interface NormalizedItem {
  id: string;
  ordinal: number;
  status: string;
  label: string;
  errorCode?: string;
  errorMessage?: string;
  videoUrl?: string;
  filePath?: string;
  assetId?: string;
  isVideo: boolean;
  recordAvailable: boolean;
}

interface Counts {
  total: number;
  pending: number;
  running: number;
  succeeded: number;
  failed: number;
  cancelled: number;
  skipped: number;
}

interface NormalizedBatch {
  id: string;
  name: string;
  status: string;
  items: NormalizedItem[];
  counts: Counts;
}

const filters: Array<{ value: ProductionMonitorFilter; label: string }> = [
  { value: "ALL", label: "全部" },
  { value: "RUNNING", label: "生成中" },
  { value: "FAILED", label: "失败" },
  { value: "COMPLETED", label: "已完成" },
];

export function ProductionMonitor({
  batch,
  readModel,
  onRetry,
  onRetryItem,
  onPlay,
  onOpenFileLocation,
  onOpenFile,
  onViewAllProducts,
  onViewAllFinishedProducts,
  onViewAllOutputs,
  onOpenProductsFolder,
  onOpenOutputFolder,
  onOpenFinishedProductsFolder,
  onExportProductList,
  onExportManifest,
  onExportProducts,
  onSelectNextProductionPackage,
  onSelectNextPackage,
  onChooseNextProductionPackage,
}: ProductionMonitorProps) {
  const [filter, setFilter] = useState<ProductionMonitorFilter>("ALL");
  const [page, setPage] = useState(0);
  const [busyAction, setBusyAction] = useState<string>();
  const model = useMemo(() => normalizeBatch(batch ?? readModel), [batch, readModel]);
  const filteredItems = useMemo(
    () => model.items.filter((item) => matchesFilter(item.status, filter)),
    [filter, model.items],
  );
  const pageCount = Math.max(1, Math.ceil(filteredItems.length / PRODUCTION_MONITOR_PAGE_SIZE));
  const safePage = Math.min(page, pageCount - 1);
  const visibleItems = filteredItems.slice(
    safePage * PRODUCTION_MONITOR_PAGE_SIZE,
    (safePage + 1) * PRODUCTION_MONITOR_PAGE_SIZE,
  );
  const terminal = model.counts.succeeded + model.counts.failed + model.counts.cancelled + model.counts.skipped;
  const terminalPercent = model.counts.total > 0 ? Math.min(100, Math.round((terminal / model.counts.total) * 100)) : 0;
  const successPercent = model.counts.total > 0 ? Math.min(100, Math.round((model.counts.succeeded / model.counts.total) * 100)) : 0;
  const isComplete = isCompletedBatch(model.status) || (model.counts.total > 0 && terminal >= model.counts.total);

  useEffect(() => {
    if (page !== safePage) setPage(safePage);
  }, [page, safePage]);

  function chooseFilter(nextFilter: ProductionMonitorFilter) {
    setFilter(nextFilter);
    setPage(0);
  }

  async function runAction(key: string, action?: () => void | Promise<void>) {
    if (!action || busyAction) return;
    setBusyAction(key);
    try {
      await action();
    } finally {
      setBusyAction(undefined);
    }
  }

  const retry = onRetryItem ?? onRetry;
  const openFile = onOpenFile ?? onOpenFileLocation;
  const viewProducts = onViewAllFinishedProducts ?? onViewAllProducts ?? onViewAllOutputs;
  const openFolder = onOpenOutputFolder ?? onOpenProductsFolder ?? onOpenFinishedProductsFolder;
  const exportList = onExportManifest ?? onExportProductList ?? onExportProducts;
  const selectNext = onSelectNextPackage ?? onSelectNextProductionPackage ?? onChooseNextProductionPackage;

  function viewAllProducts() {
    chooseFilter("COMPLETED");
    return viewProducts?.();
  }

  return (
    <section className="production-monitor" aria-label="生产监控" data-testid="production-monitor">
      <header className="production-monitor-header">
        <div>
          <p className="production-monitor-eyebrow">生产批次</p>
          <h2>{model.name}</h2>
          <p className="production-monitor-batch-id">批次 ID：{model.id}</p>
        </div>
        <div className={`production-monitor-batch-status status-${statusClass(model.status)}`}>
          <span>批次状态</span>
          <strong>{batchStatusLabel(model.status, isComplete)}</strong>
        </div>
      </header>

      <div className="production-monitor-summary" aria-label="生产摘要" data-testid="production-monitor-summary">
        <SummaryCard label="总数" value={model.counts.total} />
        <SummaryCard label="等待中" value={model.counts.pending} />
        <SummaryCard label="生成中" value={model.counts.running} />
        <SummaryCard label="成功" value={model.counts.succeeded} tone="success" />
        <SummaryCard label="失败" value={model.counts.failed} tone="danger" />
        <SummaryCard label="已取消" value={model.counts.cancelled} />
        <SummaryCard label="已跳过" value={model.counts.skipped} />
      </div>

      <div className="production-monitor-progress-row">
        <div className="production-monitor-progress-copy">
          <span>终态进度</span>
          <strong>{terminal} / {model.counts.total}</strong>
        </div>
        <div className="production-monitor-progress-track" aria-label={`终态进度 ${terminalPercent}%`}>
          <span style={{ width: `${terminalPercent}%` }} />
        </div>
        <strong className="production-monitor-progress-percent">{terminalPercent}%</strong>
        <div className="production-monitor-success-rate" aria-label={`成功率 ${successPercent}%`}>
          <span>成功率</span>
          <strong>{successPercent}%</strong>
        </div>
      </div>

      <nav className="production-monitor-filters" aria-label="项目筛选">
        {filters.map((option) => {
          const count = option.value === "ALL"
            ? model.items.length
            : model.items.filter((item) => matchesFilter(item.status, option.value)).length;
          return (
            <button
              key={option.value}
              type="button"
              className={filter === option.value ? "is-active" : undefined}
              aria-pressed={filter === option.value}
              onClick={() => chooseFilter(option.value)}
            >
              {option.label}<span>{count}</span>
            </button>
          );
        })}
      </nav>

      {visibleItems.length > 0 ? (
        <ol className="production-monitor-items" aria-label="生产项目列表">
          {visibleItems.map((item) => (
            <MonitorItem
              key={item.id}
              item={item}
              busyAction={busyAction}
              onRetry={retry ? () => void runAction(`retry:${item.id}`, () => retry(item.id)) : undefined}
              playable={item.recordAvailable && item.isVideo && Boolean(item.assetId)}
              onPlay={onPlay && item.assetId ? () => void runAction(`play:${item.id}`, () => onPlay(item.id, item.assetId!)) : undefined}
              onOpenFile={openFile && item.filePath ? () => void runAction(`open:${item.id}`, () => openFile(item.id, item.filePath)) : undefined}
            />
          ))}
        </ol>
      ) : (
        <p className="production-monitor-empty" role="status">当前筛选条件下没有项目。</p>
      )}

      <footer className="production-monitor-footer">
        <span className="production-monitor-page-label">第 {safePage + 1} / {pageCount} 页 · 每页 {PRODUCTION_MONITOR_PAGE_SIZE} 项</span>
        <div className="production-monitor-pagination" aria-label="生产项目分页">
          <button type="button" onClick={() => setPage(Math.max(0, safePage - 1))} disabled={safePage === 0}>上一页</button>
          <button type="button" onClick={() => setPage(Math.min(pageCount - 1, safePage + 1))} disabled={safePage >= pageCount - 1}>下一页</button>
        </div>
      </footer>

      {isComplete && (
        <section className="production-monitor-completion" aria-label="批次完成操作">
          <div>
            <p className="production-monitor-eyebrow">批次已结束</p>
            <h3>{model.counts.failed > 0 ? "批次已完成，部分项目失败" : "批次已完成"}</h3>
            <p>已终止 {terminal} 项，可继续查看或导出本批次成品。</p>
          </div>
          <div className="production-monitor-completion-actions">
            <ActionButton label="查看全部成品" action={() => void runAction("view-products", viewAllProducts)} busy={busyAction === "view-products"} />
            {openFolder && <ActionButton label="打开成品文件夹" action={() => runAction("open-folder", openFolder)} busy={busyAction === "open-folder"} />}
            {exportList && <ActionButton label="导出成品清单" action={() => runAction("export-list", exportList)} busy={busyAction === "export-list"} />}
            {selectNext && <ActionButton label="选择下一个生产包" action={() => runAction("select-next", selectNext)} busy={busyAction === "select-next"} primary />}
          </div>
        </section>
      )}
    </section>
  );
}

function SummaryCard({ label, value, tone }: { label: string; value: number; tone?: "success" | "danger" }) {
  return <div className={`production-monitor-summary-card${tone ? ` tone-${tone}` : ""}`}><strong>{value}</strong><span>{label}</span></div>;
}

function ActionButton({ label, action, busy, primary = false }: { label: string; action?: () => void; busy: boolean; primary?: boolean }) {
  return <button type="button" className={primary ? "primary" : undefined} onClick={action} disabled={!action || busy}>{busy ? "处理中…" : label}</button>;
}

function MonitorItem({ item, busyAction, playable, onRetry, onPlay, onOpenFile }: { item: NormalizedItem; busyAction?: string; playable: boolean; onRetry?: () => void; onPlay?: () => void; onOpenFile?: () => void }) {
  const isRetrying = busyAction === `retry:${item.id}`;
  const isPlaying = busyAction === `play:${item.id}`;
  const isOpening = busyAction === `open:${item.id}`;
  return (
    <li className="production-monitor-item" data-item-id={item.id} data-ordinal={item.ordinal}>
      <span className="production-monitor-ordinal">{item.ordinal}</span>
      <div className="production-monitor-item-main">
        <div className="production-monitor-item-heading">
          <strong>{item.label}</strong>
          <span className={`production-monitor-item-status status-${statusClass(item.status)}`}>{itemStatusLabel(item.status)}</span>
        </div>
        {item.status === "FAILED" && (
          <div className="production-monitor-error" role="alert">
            <strong>{item.errorCode ? `错误 ${item.errorCode}` : "错误详情"}</strong>
            <span>{item.errorMessage ?? "未提供错误详情"}</span>
          </div>
        )}
        {item.status === "SUCCEEDED" && (
          item.recordAvailable ? (
            <div className="production-monitor-output">
              <span className="production-monitor-output-label">成品记录可用</span>
              {playable && <button type="button" className="quiet" onClick={onPlay} disabled={!onPlay || Boolean(busyAction)}>{isPlaying ? "播放中…" : "播放"}</button>}
              {onOpenFile && <button type="button" className="quiet" onClick={onOpenFile} disabled={isOpening || Boolean(busyAction)}>{isOpening ? "打开中…" : "打开文件位置"}</button>}
            </div>
          ) : (
            <p className="production-monitor-unavailable" role="status">成品记录不可用</p>
          )
        )}
      </div>
      {item.status === "FAILED" && onRetry && (
        <button type="button" className="quiet production-monitor-retry" onClick={onRetry} disabled={Boolean(busyAction)}>
          {isRetrying ? "重试中…" : "重试"}
        </button>
      )}
    </li>
  );
}

function normalizeBatch(value?: ProductionMonitorBatchReadModel | null): NormalizedBatch {
  const source = value ?? {};
  const nestedBatch = source.batch ?? {};
  const summary = source.summary ?? {};
  const rawItems = source.items ?? source.results ?? source.entries ?? nestedBatch.items ?? nestedBatch.results ?? nestedBatch.entries ?? [];
  const items = rawItems
    .map((item, index) => normalizeItem(item, index))
    .sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id));
  const derived = deriveCounts(items);
  const counts: Counts = {
    total: firstCount(source.total, summary.total, source.totalItems, summary.totalItems, derived.total),
    pending: firstCount(source.pending, summary.pending, source.pendingItems, summary.pendingItems, derived.pending),
    running: firstCount(source.running, summary.running, source.runningItems, summary.runningItems, derived.running),
    succeeded: firstCount(source.succeeded, source.successCount, summary.succeeded, summary.succeededItems, source.succeededItems, derived.succeeded),
    failed: firstCount(source.failed, summary.failed, source.failedItems, summary.failedItems, derived.failed),
    cancelled: firstCount(source.cancelled, summary.cancelled, source.cancelledItems, summary.cancelledItems, derived.cancelled),
    skipped: firstCount(source.skipped, summary.skipped, source.skippedItems, summary.skippedItems, derived.skipped),
  };
  return {
    id: displayText(source.id ?? source.batchId ?? nestedBatch.id ?? nestedBatch.batchId) ?? "—",
    name: displayText(source.name ?? source.batchName ?? nestedBatch.name ?? nestedBatch.batchName) ?? "未命名批次",
    status: normalizeStatus(source.status ?? nestedBatch.status),
    items,
    counts: { ...counts, total: Math.max(counts.total, items.length) },
  };
}

function normalizeItem(value: ProductionMonitorItemReadModel, index: number): NormalizedItem {
  const id = displayText(value.id ?? value.itemId) ?? `item-${index + 1}`;
  const ordinal = typeof value.ordinal === "number" && Number.isFinite(value.ordinal) ? value.ordinal : index + 1;
  const status = normalizeStatus(value.status ?? value.productionItemStatus ?? value.taskStatus);
  const output = firstRecord(value.output, value.product, value.asset, value.media, value.outputAssets?.[0]);
  const videoUrl = displayText(value.videoUrl ?? output?.videoUrl);
  const outputReference = displayText(value.outputUrl ?? value.fileUrl ?? value.mediaUrl ?? value.assetUrl ?? output?.mediaUrl ?? output?.url ?? value.output);
  const filePath = displayText(value.filePath ?? value.videoPath ?? value.outputPath ?? value.outputLocation ?? value.fileLocation ?? value.localPath ?? output?.filePath ?? output?.path ?? output?.location ?? output?.localPath);
  const isVideo = isVideoOutput(value, output, videoUrl);
  const recordFlag = value.recordAvailable ?? value.productRecordAvailable;
  const assetId = displayText(value.assetId ?? value.selectedAssetId ?? output?.id);
  const recordAvailable = recordFlag !== false && Boolean(videoUrl || outputReference || filePath || assetId || output || value.outputAssets?.length);
  const nestedError = typeof value.error === "object" && value.error ? value.error : undefined;
  return {
    id,
    ordinal,
    status,
    label: displayText(value.name ?? value.title ?? value.label ?? value.shotName ?? value.sceneName ?? value.promptText) ?? `第 ${ordinal} 项`,
    errorCode: displayText(value.errorCode),
    errorMessage: displayText(value.errorMessage ?? nestedError?.message ?? nestedError?.detail ?? value.error ?? value.failureReason ?? value.errorDetail ?? value.reason ?? value.message),
    videoUrl,
    filePath,
    assetId,
    isVideo,
    recordAvailable,
  };
}

function isVideoOutput(value: ProductionMonitorItemReadModel, output: Record<string, unknown> | undefined, videoUrl?: string): boolean {
  const mediaType = displayText(value.assetType ?? value.mediaType ?? value.mimeType ?? output?.assetType ?? output?.mediaType ?? output?.mimeType);
  if (mediaType) return mediaType.toLowerCase().includes("video");
  return Boolean(videoUrl);
}

function deriveCounts(items: readonly NormalizedItem[]): Counts {
  const counts: Counts = { total: items.length, pending: 0, running: 0, succeeded: 0, failed: 0, cancelled: 0, skipped: 0 };
  items.forEach((item) => {
    if (item.status === "PENDING") counts.pending += 1;
    else if (isRunningStatus(item.status)) counts.running += 1;
    else if (item.status === "SUCCEEDED") counts.succeeded += 1;
    else if (item.status === "FAILED") counts.failed += 1;
    else if (item.status === "CANCELLED") counts.cancelled += 1;
    else if (item.status === "SKIPPED") counts.skipped += 1;
  });
  return counts;
}

function firstCount(...values: Array<number | null | undefined>): number {
  const value = values.find((candidate) => typeof candidate === "number" && Number.isFinite(candidate));
  return Math.max(0, Math.floor(value ?? 0));
}

function firstRecord(...values: unknown[]): Record<string, unknown> | undefined {
  return values.find((value): value is Record<string, unknown> => Boolean(value) && typeof value === "object" && !Array.isArray(value));
}

function displayText(value: unknown): string | undefined {
  if (typeof value !== "string" && typeof value !== "number") return undefined;
  const text = String(value).trim();
  return text || undefined;
}

function normalizeStatus(value: unknown): string {
  const status = displayText(value)?.toUpperCase() ?? "PENDING";
  if (["READY", "QUEUED", "WAITING"].includes(status)) return "PENDING";
  if (["DISPATCHING", "DISPATCHED", "ACTIVE", "GENERATING", "IN_PROGRESS"].includes(status)) return "RUNNING";
  if (["SUCCESS", "DONE", "COMPLETED"].includes(status)) return "SUCCEEDED";
  return status;
}

function matchesFilter(status: string, filter: ProductionMonitorFilter): boolean {
  if (filter === "ALL") return true;
  if (filter === "RUNNING") return isRunningStatus(status);
  if (filter === "FAILED") return status === "FAILED";
  return status === "SUCCEEDED";
}

function isRunningStatus(status: string): boolean {
  return status === "RUNNING";
}

function isCompletedBatch(status: string): boolean {
  return ["COMPLETED", "SUCCEEDED", "DONE", "FAILED", "CANCELLED"].includes(status);
}

function statusClass(status: string): string {
  return status.toLowerCase().replace(/[^a-z0-9_-]/g, "-");
}

function itemStatusLabel(status: string): string {
  return {
    PENDING: "等待中",
    RUNNING: "生成中",
    PAUSED: "已暂停",
    SUCCEEDED: "已完成",
    FAILED: "失败",
    CANCELLED: "已取消",
    SKIPPED: "已跳过",
  }[status] ?? "处理中";
}

function batchStatusLabel(status: string, complete: boolean): string {
  if (complete) return "已完成";
  return { PENDING: "等待中", RUNNING: "生成中", PAUSED: "已暂停", FAILED: "失败", CANCELLED: "已取消" }[status] ?? itemStatusLabel(status);
}

export function filterProductionMonitorItems(items: readonly ProductionMonitorItemReadModel[], filter: ProductionMonitorFilter): ProductionMonitorItemReadModel[] {
  return items
    .map((item, index) => normalizeItem(item, index))
    .filter((item) => matchesFilter(item.status, filter))
    .sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id));
}
