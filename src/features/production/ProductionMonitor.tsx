import { useEffect, useMemo, useState } from "react";
import { isComfyNodeIncompatible, toUserMessage } from "../../i18n/errorMessages";
import type { ProductionBatchArtifactsDto, ProductionTaskDto } from "../../types/artifact";
import { ArtifactActions, ArtifactMetadata, ArtifactPreview, ReviewBadge } from "./ArtifactComponents";
import "./ProductionMonitor.css";

export const PRODUCTION_MONITOR_PAGE_SIZE = 50;
export type ProductionMonitorFilter = "ALL" | "RUNNING" | "FAILED" | "COMPLETED";

export interface ProductionMonitorProps {
  projectId: string;
  batch?: ProductionBatchArtifactsDto | null;
  loading?: boolean;
  error?: string;
  onRetry?: (productionItemId: string) => void | Promise<void>;
  onViewAllProducts?: () => void | Promise<void>;
  onExportManifest?: () => void | Promise<void>;
  onSelectNextProductionPackage?: () => void | Promise<void>;
}

const FILTERS: Array<{ value: ProductionMonitorFilter; label: string }> = [
  { value: "ALL", label: "全部" },
  { value: "RUNNING", label: "运行中" },
  { value: "FAILED", label: "失败" },
  { value: "COMPLETED", label: "已完成" },
];

export function ProductionMonitor({
  projectId,
  batch,
  loading = false,
  error,
  onRetry,
  onViewAllProducts,
  onExportManifest,
  onSelectNextProductionPackage,
}: ProductionMonitorProps) {
  const [filter, setFilter] = useState<ProductionMonitorFilter>("ALL");
  const [page, setPage] = useState(0);
  const [busyItem, setBusyItem] = useState<string>();
  const items = batch?.items ?? [];
  const filtered = useMemo(() => items.filter((item) => matchesFilter(item.productionItemStatus, filter)), [filter, items]);
  const pageCount = Math.max(1, Math.ceil(filtered.length / PRODUCTION_MONITOR_PAGE_SIZE));
  const safePage = Math.min(page, pageCount - 1);
  const visible = filtered.slice(safePage * PRODUCTION_MONITOR_PAGE_SIZE, (safePage + 1) * PRODUCTION_MONITOR_PAGE_SIZE);
  const terminal = (batch?.succeeded ?? 0) + (batch?.failed ?? 0) + (batch?.cancelled ?? 0) + (batch?.skipped ?? 0);
  const total = batch?.total ?? 0;
  const terminalPercent = total > 0 ? Math.min(100, Math.round(terminal / total * 100)) : 0;
  const successPercent = total > 0 ? Math.min(100, Math.round((batch?.succeeded ?? 0) / total * 100)) : 0;
  const isComplete = Boolean(batch && (isCompletedBatch(batch.status) || (total > 0 && terminal >= total)));
  const hasFailures = Boolean(batch && (batch.status === "FAILED" || batch.failed > 0));

  useEffect(() => { if (page !== safePage) setPage(safePage); }, [page, safePage]);

  async function retry(item: ProductionTaskDto) {
    if (!onRetry || busyItem) return;
    setBusyItem(item.productionItemId);
    try { await onRetry(item.productionItemId); }
    finally { setBusyItem(undefined); }
  }

  if (loading && !batch) return <p className="project-loading" role="status">正在加载生产与产物记录…</p>;
  if (!batch) return <p className="production-monitor-empty" role={error ? "alert" : "status"}>{error ?? "选择一个生产批次以查看执行任务和产物。"}</p>;

  return (
    <section className="production-monitor" aria-label="生产监控" data-testid="production-monitor">
      <header className="production-monitor-header">
        <div><p className="production-monitor-eyebrow">生产批次</p><h2>{batch.batchName}</h2><p className="production-monitor-batch-id">批次 ID：{batch.batchId}</p></div>
        <div className={`production-monitor-batch-status status-${statusClass(batch.status)}`}><span>批次状态</span><strong>{batchStatusLabel(batch.status, isComplete, hasFailures)}</strong></div>
      </header>

      {error && <p className="production-monitor-error" role="alert">{error}</p>}
      {loading && <p className="production-monitor-loading" role="status">正在刷新生产记录…</p>}

      <div className="production-monitor-summary" aria-label="生产摘要" data-testid="production-monitor-summary">
        <SummaryCard label="总数" value={batch.total} />
        <SummaryCard label="待执行" value={batch.pending} />
        <SummaryCard label="运行中" value={batch.running} />
        <SummaryCard label="成功" value={batch.succeeded} tone="success" />
        <SummaryCard label="失败" value={batch.failed} tone="danger" />
        <SummaryCard label="已取消" value={batch.cancelled} />
        <SummaryCard label="已跳过" value={batch.skipped} />
      </div>

      {batch.status === "PAUSED" && items.some((item) => item.productionItemStatus === "FAILED" && isComfyNodeIncompatible(item.errorCode, item.errorMessage)) && (
        <p role="alert">已暂停：工作流与当前 ComfyUI 不兼容，剩余 {batch.pending} 项尚未执行。请先修复运行环境或使用兼容工作流，再继续生产；不会自动重试。</p>
      )}

      <div className="production-monitor-progress-row">
        <div className="production-monitor-progress-copy"><span>终态进度</span><strong>{terminal} / {total}</strong></div>
        <div className="production-monitor-progress-track" aria-label={`终态进度 ${terminalPercent}%`}><span style={{ width: `${terminalPercent}%` }} /></div>
        <strong className="production-monitor-progress-percent">{terminalPercent}%</strong>
        <div className="production-monitor-success-rate" aria-label={`成功率 ${successPercent}%`}><span>成功率</span><strong>{successPercent}%</strong></div>
      </div>

      <nav className="production-monitor-filters" aria-label="生产任务筛选">
        {FILTERS.map((option) => {
          const count = option.value === "ALL" ? items.length : items.filter((item) => matchesFilter(item.productionItemStatus, option.value)).length;
          return <button key={option.value} type="button" className={filter === option.value ? "is-active" : undefined} aria-pressed={filter === option.value} onClick={() => { setFilter(option.value); setPage(0); }}>{option.label}<span>{count}</span></button>;
        })}
      </nav>

      {visible.length ? (
        <ol className="production-monitor-items" aria-label="生产任务列表">
          {visible.map((item) => (
            <TaskRow key={item.productionItemId} projectId={projectId} item={item} busy={busyItem === item.productionItemId || Boolean(busyItem)} onRetry={onRetry ? () => void retry(item) : undefined} />
          ))}
        </ol>
      ) : <p className="production-monitor-empty" role="status">当前筛选条件下没有生产任务。</p>}

      <footer className="production-monitor-footer">
        <span className="production-monitor-page-label">第 {safePage + 1} / {pageCount} 页 · 每页 {PRODUCTION_MONITOR_PAGE_SIZE} 项</span>
        <div className="production-monitor-pagination" aria-label="生产任务分页">
          <button type="button" onClick={() => setPage(Math.max(0, safePage - 1))} disabled={safePage === 0}>上一页</button>
          <button type="button" onClick={() => setPage(Math.min(pageCount - 1, safePage + 1))} disabled={safePage >= pageCount - 1}>下一页</button>
        </div>
      </footer>

      {isComplete && <section className="production-monitor-completion" aria-label="批次完成操作">
        <div><p className="production-monitor-eyebrow">批次已结束</p><h3>{batch.status === "CANCELLED" ? "批次已取消" : hasFailures ? (batch.succeeded === 0 ? "生成失败，没有成功结果" : "批次已结束，存在失败项目") : "批次已完成"}</h3><p>{hasFailures ? `已处理 ${terminal} 项：成功 ${batch.succeeded} 项，失败 ${batch.failed} 项。失败项目需修复后重试。` : `已处理 ${terminal} 项，可继续查看或导出已登记产物。`}</p></div>
        <div className="production-monitor-completion-actions">
          {batch.succeeded > 0 && <ActionButton label="查看已完成产物" action={() => { setFilter("COMPLETED"); setPage(0); void onViewAllProducts?.(); }} />}
          {onExportManifest && !hasFailures && <ActionButton label="导出产物清单" action={() => onExportManifest()} />}
          {onSelectNextProductionPackage && !hasFailures && <ActionButton label="选择下一个生产包" action={() => onSelectNextProductionPackage()} primary />}
        </div>
      </section>}
    </section>
  );
}

function TaskRow({ projectId, item, busy, onRetry }: { projectId: string; item: ProductionTaskDto; busy: boolean; onRetry?: () => void }) {
  const failed = item.productionItemStatus === "FAILED";
  const incompatible = failed && isComfyNodeIncompatible(item.errorCode, item.errorMessage);
  return (
    <li className="production-monitor-item" data-item-id={item.productionItemId} data-ordinal={item.ordinal}>
      <span className="production-monitor-ordinal">{item.ordinal + 1}</span>
      <div className="production-monitor-item-main">
        <div className="production-monitor-item-heading"><strong>生产项 {item.ordinal + 1}</strong><span className={`production-monitor-item-status status-${statusClass(item.productionItemStatus)}`}>{itemStatusLabel(item.productionItemStatus)}</span></div>
        {item.task ? <p className="production-monitor-task-context">Generation Task：{item.task.id} · {itemStatusLabel(item.task.status)} · {item.task.workflowVersionId} / {item.task.recipeId}</p> : <p className="production-monitor-task-context">尚未创建 Generation Task</p>}
        {failed && <div className="production-monitor-error" role="alert"><strong>{item.errorCode ? `错误 ${item.errorCode}` : "错误详情"}</strong>{incompatible ? <><span>{toUserMessage({ code: item.errorCode, message: item.errorMessage })}</span><details><summary>查看技术详情</summary><pre>{item.errorMessage}</pre></details></> : <span>{item.errorMessage ?? "未提供错误详情"}</span>}<small>{onRetry ? "失败，需要处理；可创建新的执行尝试，原失败记录会保留。" : "失败，需要处理；请查看错误详情。"}</small></div>}
        {item.productionItemStatus === "SUCCEEDED" && item.artifacts.length === 0 && <p className="production-monitor-unavailable" role="status">任务已完成，但没有登记可用产物。</p>}
        {item.artifacts.length > 0 && <section className="production-artifact-list" aria-label={`生产项 ${item.ordinal + 1} 的产物`}>
          {item.artifacts.map((artifact) => <article className="production-artifact-card" key={artifact.id} data-artifact-id={artifact.id}>
            <div className="production-artifact-card-heading"><strong>{artifact.name}</strong><ReviewBadge decision={artifact.reviewStatus} /></div>
            <ArtifactPreview projectId={projectId} artifact={artifact} />
            <ArtifactMetadata artifact={artifact} />
            <ArtifactActions artifact={artifact} />
          </article>)}
        </section>}
      </div>
      {failed && onRetry && <button type="button" className="quiet production-monitor-retry" onClick={onRetry} disabled={busy}>{busy ? "重试中…" : incompatible ? "修复后重试" : "重试"}</button>}
    </li>
  );
}

function SummaryCard({ label, value, tone }: { label: string; value: number; tone?: "success" | "danger" }) {
  return <div className={`production-monitor-summary-card${tone ? ` tone-${tone}` : ""}`}><strong>{value}</strong><span>{label}</span></div>;
}

function ActionButton({ label, action, primary = false }: { label: string; action: () => void | Promise<void>; primary?: boolean }) {
  return <button type="button" className={primary ? "primary" : undefined} onClick={() => void action()}>{label}</button>;
}

function matchesFilter(status: string, filter: ProductionMonitorFilter): boolean {
  if (filter === "ALL") return true;
  if (filter === "RUNNING") return ["RUNNING", "DISPATCHING", "DISPATCHED"].includes(status);
  if (filter === "FAILED") return status === "FAILED";
  return status === "SUCCEEDED";
}

function isCompletedBatch(status: string): boolean {
  return ["COMPLETED", "SUCCEEDED", "DONE", "FAILED", "CANCELLED"].includes(status);
}

function statusClass(status: string): string { return status.toLowerCase().replace(/[^a-z0-9_-]/g, "-"); }

function itemStatusLabel(status: string): string {
  return ({ READY: "待执行", PENDING: "待执行", DISPATCHING: "提交中", DISPATCHED: "已提交", RUNNING: "运行中", PAUSED: "已暂停", SUCCEEDED: "已完成", COMPLETED: "已完成", FAILED: "失败，需要处理", CANCELLED: "已取消", SKIPPED: "已跳过" } as Record<string, string>)[status] ?? status;
}

function batchStatusLabel(status: string, complete: boolean, hasFailures: boolean): string {
  if (status === "FAILED") return "失败，需要处理";
  if (status === "CANCELLED") return "已取消";
  if (complete && hasFailures) return "已结束，有失败项目";
  if (complete) return "已完成";
  return ({ READY: "待启动", RUNNING: "运行中", PAUSED: "已暂停", COMPLETED: "已完成" } as Record<string, string>)[status] ?? status;
}
