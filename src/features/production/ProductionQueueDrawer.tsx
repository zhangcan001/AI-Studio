import { useId, useMemo, useState } from "react";
import type {
  ProductionBatchDetail,
  ProductionBatchItemView,
  ProductionBatchSummary,
  ProductionQueueOverview,
} from "../../types/productionQueue";
import type { ProductionBatchRunbookRow, ProductionBatchRunbookView } from "../../types/productionBatchRunbook";
import "./ProductionQueueDrawer.css";

const MAX_VISIBLE_ROWS = 24;

/** Optional presentation fields accepted from an existing item view when available. */
export interface ProductionQueueDrawerItem extends ProductionBatchItemView {
  shotName?: string | null;
  stage?: string | null;
  thumbnailUrl?: string | null;
  resolution?: string | null;
  duration?: string | number | null;
}

/**
 * QueueDrawer is deliberately a presentational boundary. It never fetches queue
 * data or invokes a backend command; the host owns those effects and passes the
 * current overview/details/items/runbook snapshot plus one-item callbacks.
 */
export interface ProductionQueueDrawerProps {
  overview?: ProductionQueueOverview | null;
  details?: readonly ProductionBatchDetail[];
  items?: readonly ProductionQueueDrawerItem[];
  runbook?: ProductionBatchRunbookView | null;
  /** Existing queue summaries are a fallback for hosts that have not loaded details yet. */
  queues?: readonly ProductionBatchSummary[];
  /** Controlled expansion; omit it to use the collapsed-by-default local state. */
  expanded?: boolean;
  defaultExpanded?: boolean;
  onToggle?: (expanded: boolean) => void;
  onStart?: (batchId: string) => void | Promise<void>;
  onPause?: (batchId: string) => void | Promise<void>;
  onRetry?: (itemId: string) => void | Promise<void>;
  onOpen?: (batchId: string) => void | Promise<void>;
}

interface QueueDisplayData {
  thumbnailUrl?: string;
  shotName?: string;
  stage?: string;
  resolution?: string;
  duration?: string;
}

interface DrawerBatchRow extends QueueDisplayData {
  id: string;
  name: string;
  status: string;
  archivedAt?: string;
  total?: number;
  pending?: number;
  active?: number;
  succeeded?: number;
  failed?: number;
  readyToStart?: boolean;
  blockedReason?: string | null;
  mixedScope?: boolean;
}

interface QueueStats {
  running?: number;
  pending?: number;
  failed?: number;
}

type ActionKind = "start" | "pause" | "retry" | "open";

export function ProductionQueueDrawer({
  overview,
  details = [],
  items,
  runbook,
  queues = [],
  expanded,
  defaultExpanded = false,
  onToggle,
  onStart,
  onPause,
  onRetry,
  onOpen,
}: ProductionQueueDrawerProps) {
  const [localExpanded, setLocalExpanded] = useState(defaultExpanded);
  const [busyAction, setBusyAction] = useState<string>();
  const drawerId = useId().replace(/:/g, "");
  const contentId = `production-queue-drawer-${drawerId}-content`;
  const isExpanded = expanded ?? localExpanded;
  const rows = useMemo(() => buildRows(details, queues, runbook), [details, queues, runbook]);
  const visibleRows = rows.slice(0, MAX_VISIBLE_ROWS);
  const itemRows = useMemo(
    () => items?.length ? [...items] : details.flatMap((detail) => detail.items),
    [details, items],
  );
  const visibleItems = itemRows.slice(0, MAX_VISIBLE_ROWS);
  const stats = useMemo(
    () => getQueueStats(overview, details, queues, runbook, rows),
    [details, overview, queues, runbook, rows],
  );

  function toggleExpanded() {
    const next = !isExpanded;
    if (expanded === undefined) setLocalExpanded(next);
    onToggle?.(next);
  }

  async function runAction(kind: ActionKind, id: string, handler?: (value: string) => void | Promise<void>) {
    if (!handler || busyAction) return;
    setBusyAction(`${kind}:${id}`);
    try {
      await handler(id);
    } finally {
      setBusyAction(undefined);
    }
  }

  return (
    <section className="production-queue-drawer" data-expanded={isExpanded} aria-label="生产队列">
      <button
        type="button"
        className="production-queue-drawer-toggle"
        aria-expanded={isExpanded}
        aria-controls={contentId}
        onClick={toggleExpanded}
      >
        <span className="production-queue-drawer-title">
          <span className="production-queue-drawer-kicker">QUEUE</span>
          <strong>生产队列</strong>
        </span>
        <span className="production-queue-drawer-stats" aria-label="生产队列摘要">
          <span><strong>{formatCount(stats.running)}</strong><small>运行中</small></span>
          <span><strong>{formatCount(stats.pending)}</strong><small>等待中</small></span>
          <span><strong>{formatCount(stats.failed)}</strong><small>失败</small></span>
        </span>
        <span className="production-queue-drawer-chevron" aria-hidden="true">{isExpanded ? "⌄" : "⌃"}</span>
      </button>

      {isExpanded && (
        <div id={contentId} className="production-queue-drawer-body">
          {visibleRows.length ? (
            <ul className="production-queue-drawer-list" aria-label="生产队列批次">
              {visibleRows.map((row) => (
                <BatchRow
                  key={row.id}
                  row={row}
                  busyAction={busyAction}
                  onStart={onStart ? (id) => void runAction("start", id, onStart) : undefined}
                  onPause={onPause ? (id) => void runAction("pause", id, onPause) : undefined}
                  onRetry={onRetry ? (id) => void runAction("retry", id, onRetry) : undefined}
                  onOpen={onOpen ? (id) => void runAction("open", id, onOpen) : undefined}
                />
              ))}
            </ul>
          ) : (
            <p className="production-queue-drawer-empty" role="status">当前没有可显示的队列数据。</p>
          )}

          {rows.length > visibleRows.length && (
            <p className="production-queue-drawer-limit" role="status">
              已显示前 {visibleRows.length} 个队列项，共 {rows.length} 个；完整 Runbook 保持在生产工作区。
            </p>
          )}

          {visibleItems.length > 0 && (
            <div className="production-queue-drawer-items" aria-label="队列项目">
              <div className="production-queue-drawer-subheading">
                <span>项目</span>
                <small>{itemRows.length} 项 · 仅显示当前传入数据</small>
              </div>
              <ol>
                {visibleItems.map((item) => (
                  <ItemRow
                    key={item.id}
                    item={item}
                    busyAction={busyAction}
                    onRetry={onRetry ? (id) => void runAction("retry", id, onRetry) : undefined}
                  />
                ))}
              </ol>
            </div>
          )}
        </div>
      )}
    </section>
  );
}

function BatchRow({
  row,
  busyAction,
  onStart,
  onPause,
  onRetry,
  onOpen,
}: {
  row: DrawerBatchRow;
  busyAction?: string;
  onStart?: (id: string) => void;
  onPause?: (id: string) => void;
  onRetry?: (id: string) => void;
  onOpen?: (id: string) => void;
}) {
  const progress = progressFor(row);
  const isBusy = Boolean(busyAction?.endsWith(`:${row.id}`));
  const startable = isStartable(row);
  const retryable = row.status === "FAILED";

  return (
    <li className="production-queue-drawer-row" data-batch-id={row.id}>
      <Thumbnail data={row} label={row.name} />
      <div className="production-queue-drawer-row-copy">
        <div className="production-queue-drawer-row-heading">
          <strong title={row.shotName ?? row.name}>{row.shotName ?? row.name}</strong>
          <StatusChip status={row.status} />
        </div>
        <div className="production-queue-drawer-row-meta">
          {row.stage && <span>{stageLabel(row.stage)}</span>}
          {row.resolution && <span>{row.resolution}</span>}
          {row.duration && <span>{row.duration}</span>}
          {row.total !== undefined && <span>{row.succeeded ?? 0}/{row.total}</span>}
        </div>
        {row.name !== row.shotName && <small className="production-queue-drawer-row-name" title={row.name}>{row.name}</small>}
        {progress !== undefined && (
          <div className="production-queue-drawer-progress" aria-label={`完成 ${progress}%`}>
            <span><i style={{ width: `${progress}%` }} /></span>
            <small>{progress}%</small>
          </div>
        )}
      </div>
      <div className="production-queue-drawer-actions">
        {onStart && startable && (
          <button
            type="button"
            data-action="start"
            aria-label={`Start queue ${row.id}`}
            onClick={() => onStart(row.id)}
            disabled={Boolean(busyAction)}
          >
            {row.status === "PAUSED" ? "继续" : "开始"}
          </button>
        )}
        {onPause && row.status === "RUNNING" && (
          <button
            type="button"
            className="quiet"
            data-action="pause"
            aria-label={`Pause queue ${row.id}`}
            onClick={() => onPause(row.id)}
            disabled={Boolean(busyAction)}
          >
            暂停
          </button>
        )}
        {onRetry && retryable && (
          <button
            type="button"
            className="quiet"
            data-action="retry"
            aria-label={`Retry queue ${row.id}`}
            onClick={() => onRetry(row.id)}
            disabled={Boolean(busyAction)}
          >
            重试
          </button>
        )}
        {onOpen && (
          <button
            type="button"
            className="quiet"
            data-action="open"
            aria-label={`Open queue ${row.id}`}
            onClick={() => onOpen(row.id)}
            disabled={Boolean(busyAction)}
          >
            打开
          </button>
        )}
        {isBusy && <span className="production-queue-drawer-busy" role="status">处理中…</span>}
      </div>
    </li>
  );
}

function ItemRow({
  item,
  busyAction,
  onRetry,
}: {
  item: ProductionQueueDrawerItem;
  busyAction?: string;
  onRetry?: (id: string) => void;
}) {
  const data = readDisplayData(item);
  const retryable = item.status === "FAILED" || item.status === "CANCELLED";
  const isBusy = busyAction === `retry:${item.id}`;
  const itemLabel = data.shotName ?? item.promptText?.trim() ?? `第 ${item.ordinal + 1} 项`;

  return (
    <li className="production-queue-drawer-item" data-item-id={item.id}>
      <Thumbnail data={data} label={itemLabel} />
      <div className="production-queue-drawer-item-copy">
        <strong title={itemLabel}>{itemLabel}</strong>
        <div className="production-queue-drawer-row-meta">
          {data.stage && <span>{stageLabel(data.stage)}</span>}
          {data.resolution && <span>{data.resolution}</span>}
          {data.duration && <span>{data.duration}</span>}
          <StatusChip status={item.status} />
        </div>
      </div>
      {onRetry && retryable && (
        <button
          type="button"
          className="quiet"
          data-action="retry"
          aria-label={`Retry item ${item.id}`}
          onClick={() => onRetry(item.id)}
          disabled={Boolean(busyAction)}
        >
          {isBusy ? "处理中…" : "重试"}
        </button>
      )}
    </li>
  );
}

function Thumbnail({ data, label }: { data: QueueDisplayData; label: string }) {
  return (
    <div className="production-queue-drawer-thumbnail" aria-label={data.thumbnailUrl ? `${label} 缩略图` : "暂无缩略图"}>
      {data.thumbnailUrl ? <img src={data.thumbnailUrl} alt={`${label} 缩略图`} /> : <span aria-hidden="true">—</span>}
    </div>
  );
}

function StatusChip({ status }: { status: string }) {
  return <span className={`production-queue-drawer-status status-${status.toLowerCase()}`}>{statusLabel(status)}</span>;
}

function buildRows(
  details: readonly ProductionBatchDetail[],
  queues: readonly ProductionBatchSummary[],
  runbook?: ProductionBatchRunbookView | null,
): DrawerBatchRow[] {
  if (details.length) return details.map(detailToRow);
  if (queues.length) return queues.map(summaryToRow);
  return (runbook?.rows ?? []).map(runbookToRow);
}

function detailToRow(detail: ProductionBatchDetail): DrawerBatchRow {
  return {
    id: detail.id,
    name: detail.name,
    status: detail.status,
    archivedAt: detail.archivedAt,
    total: detail.total,
    pending: detail.pending,
    active: detail.running,
    succeeded: detail.succeeded,
    failed: detail.failed,
    ...readDisplayData(detail),
  };
}

function summaryToRow(queue: ProductionBatchSummary): DrawerBatchRow {
  return {
    id: queue.id,
    name: queue.name,
    status: queue.status,
    archivedAt: queue.archivedAt,
    ...readDisplayData(queue),
  };
}

function runbookToRow(row: ProductionBatchRunbookRow): DrawerBatchRow {
  const display = readDisplayData(row);
  return {
    id: row.batchId,
    name: row.batchName,
    status: row.batchStatus,
    total: row.shotCount,
    pending: row.pending,
    active: row.active,
    succeeded: row.succeeded,
    failed: row.failed,
    ...display,
    stage: display.stage ?? row.stage ?? undefined,
    shotName: display.shotName ?? row.sceneName ?? undefined,
    readyToStart: row.readyToStart,
    blockedReason: row.blockedReason,
    mixedScope: row.mixedScope,
  };
}

function getQueueStats(
  overview: ProductionQueueOverview | null | undefined,
  details: readonly ProductionBatchDetail[],
  queues: readonly ProductionBatchSummary[],
  runbook: ProductionBatchRunbookView | null | undefined,
  rows: readonly DrawerBatchRow[],
): QueueStats {
  if (overview) {
    return { running: overview.runningQueues, pending: overview.pendingItems, failed: overview.failedItems };
  }
  if (runbook?.summary) {
    return { running: runbook.summary.runningBatches, pending: runbook.summary.pending, failed: runbook.summary.failed };
  }
  if (details.length) {
    return {
      running: details.filter((detail) => detail.status === "RUNNING").length,
      pending: details.reduce((total, detail) => total + detail.pending, 0),
      failed: details.reduce((total, detail) => total + detail.failed, 0),
    };
  }
  if (runbook?.rows.length) {
    return {
      running: runbook.rows.filter((row) => row.batchStatus === "RUNNING").length,
      pending: runbook.rows.reduce((total, row) => total + row.pending, 0),
      failed: runbook.rows.reduce((total, row) => total + row.failed, 0),
    };
  }
  if (queues.length) return { running: queues.filter((queue) => queue.status === "RUNNING").length };
  if (rows.length) return { running: rows.filter((row) => row.status === "RUNNING").length };
  return {};
}

function isStartable(row: DrawerBatchRow): boolean {
  if (row.archivedAt || row.status === "RUNNING" || row.status === "COMPLETED" || row.status === "FAILED") return false;
  if (row.status !== "READY" && row.status !== "PAUSED") return false;
  return row.status === "PAUSED" || (row.readyToStart !== false && !row.blockedReason && !row.mixedScope);
}

function progressFor(row: DrawerBatchRow): number | undefined {
  if (row.total === undefined || row.total <= 0 || row.succeeded === undefined) return undefined;
  return Math.max(0, Math.min(100, Math.round((row.succeeded / row.total) * 100)));
}

function readDisplayData(value: unknown): QueueDisplayData {
  if (!value || typeof value !== "object") return {};
  const candidate = value as Record<string, unknown>;
  return {
    thumbnailUrl: displayText(candidate.thumbnailUrl ?? candidate.thumbnail),
    shotName: displayText(candidate.shotName),
    stage: displayText(candidate.stage),
    resolution: displayText(candidate.resolution),
    duration: displayText(candidate.duration),
  };
}

function displayText(value: unknown): string | undefined {
  if (typeof value === "number") return String(value);
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed || undefined;
}

function statusLabel(status: string): string {
  return {
    READY: "待启动",
    RUNNING: "运行中",
    PAUSED: "已暂停",
    COMPLETED: "已完成",
    PENDING: "等待中",
    DISPATCHING: "提交中",
    DISPATCHED: "执行中",
    SUCCEEDED: "成功",
    FAILED: "失败",
    CANCELLED: "已取消",
    SKIPPED: "已跳过",
  }[status] ?? status;
}

function stageLabel(stage: string): string {
  return { image: "图片", video: "视频" }[stage] ?? stage;
}

function formatCount(value: number | undefined): string | number {
  return value === undefined ? "—" : value;
}
