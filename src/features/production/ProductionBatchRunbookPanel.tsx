import { useMemo, useState } from "react";
import type {
  ProductionBatchRunbookFilter,
  ProductionBatchRunbookPanelProps,
  ProductionBatchRunbookRow,
  ProductionBatchRunbookStatus,
} from "../../types/productionBatchRunbook";
import { sceneProductionStageLabel } from "../../types/sceneProduction";
import "./ProductionBatchRunbookPanel.css";

const FILTERS: ProductionBatchRunbookFilter[] = ["active", "ready", "running", "paused", "completed", "all"];

export function ProductionBatchRunbookPanel({
  projectId,
  runbook,
  onRefresh,
  onStartBatch,
  onOpenProductionQueue,
  onNavigateToScene,
  onNavigateToEpisode,
}: ProductionBatchRunbookPanelProps) {
  const [filter, setFilter] = useState<ProductionBatchRunbookFilter>("active");
  const [busyBatchId, setBusyBatchId] = useState<string>();
  const rows = useMemo(() => sortRunbookRows(filterRunbookRows(runbook.rows, filter)), [filter, runbook.rows]);
  const runningRow = runbook.rows.find((row) => row.batchStatus === "RUNNING");
  const isBusy = Boolean(busyBatchId);

  async function startBatch(row: ProductionBatchRunbookRow) {
    if (!onStartBatch || !canStartRunbookRow(row, Boolean(runningRow)) || busyBatchId) return;
    setBusyBatchId(row.batchId);
    try {
      await onStartBatch(row.batchId);
      await onRefresh?.();
    } finally {
      setBusyBatchId(undefined);
    }
  }

  return (
    <section className="production-batch-runbook-panel" aria-label="Production Batch Runbook" aria-busy={isBusy} data-project-id={projectId}>
      <div className="production-batch-runbook-header">
        <div><span className="section-label">Production Batch Runbook</span><h3>生产批次执行清单</h3><p>Runbook 只读派生现有 Batch；每次只允许手动启动一个 Batch。</p></div>
        <button type="button" className="quiet-button" onClick={() => void onRefresh?.()} disabled={isBusy || !onRefresh}>{isBusy ? "处理中…" : "刷新 Runbook"}</button>
      </div>

      {runningRow && <div className="production-batch-runbook-running" role="status"><strong>当前正在生产</strong><span>第 {ordinalLabel(runningRow.episodeOrdinal)} 集 / 场景 {ordinalLabel(runningRow.sceneOrdinal)} / {runbookStageLabel(runningRow.stage)}</span><button type="button" className="quiet-button" onClick={() => onOpenProductionQueue?.(runningRow.batchId)} disabled={isBusy || !onOpenProductionQueue}>打开队列</button></div>}

      <div className="production-batch-runbook-filters" role="group" aria-label="Runbook 筛选">{FILTERS.map((value) => <button key={value} type="button" className={filter === value ? "active" : ""} onClick={() => setFilter(value)} disabled={isBusy}>{runbookFilterLabel(value)}</button>)}</div>

      <div className="production-batch-runbook-table-wrap"><table><thead><tr><th>顺序</th><th>Episode</th><th>Scene</th><th>Stage</th><th>Batch</th><th>Shots</th><th>Status</th><th>Progress</th><th>Action</th></tr></thead><tbody>{rows.map((row) => {
        const canStart = canStartRunbookRow(row, Boolean(runningRow)) && Boolean(onStartBatch);
        const recommended = row.batchId === runbook.recommendedBatchId;
        return <tr key={row.batchId} className={recommended ? "recommended" : undefined}>
          <td><span className="production-batch-runbook-order">E{ordinalLabel(row.episodeOrdinal)} · S{ordinalLabel(row.sceneOrdinal)}</span></td>
          <td><button type="button" className="production-batch-runbook-link" onClick={() => row.episodeId && onNavigateToEpisode?.(row.episodeId)} disabled={isBusy || !onNavigateToEpisode || !row.episodeId}>{row.episodeName ?? "未知 Episode"}</button></td>
          <td><button type="button" className="production-batch-runbook-link" onClick={() => row.sceneId && onNavigateToScene?.(row.sceneId)} disabled={isBusy || !onNavigateToScene || !row.sceneId}>{row.sceneName ?? "混合范围"}</button></td>
          <td>{runbookStageLabel(row.stage)}</td>
          <td><strong>{row.batchName}</strong>{recommended && <span className="production-batch-runbook-badge">建议下一批</span>}{row.mixedScope && <span className="production-batch-runbook-warning">MIXED_SCOPE</span>}{row.blockedReason && <small className="production-batch-runbook-blocker">{row.blockedReason}</small>}</td>
          <td>{row.shotCount}</td>
          <td><span className={`production-batch-runbook-status production-batch-runbook-status-${row.batchStatus.toLowerCase()}`}>{runbookStatusLabel(row.batchStatus)}</span></td>
          <td><div className="production-batch-runbook-progress" aria-label={`${runbookProgress(row)}%`}><span style={{ width: `${runbookProgress(row)}%` }} /><small>{row.succeeded}/{row.shotCount}</small></div></td>
          <td><div className="production-batch-runbook-actions"><button type="button" onClick={() => void startBatch(row)} disabled={!canStart || isBusy}>{busyBatchId === row.batchId ? "启动中…" : "启动"}</button><button type="button" className="quiet-button" onClick={() => onOpenProductionQueue?.(row.batchId)} disabled={isBusy || !onOpenProductionQueue}>打开队列</button></div></td>
        </tr>;
      })}</tbody></table></div>
      {!rows.length && <div className="production-batch-runbook-empty"><strong>当前筛选没有 Batch</strong><span>Generic Batch 不属于 Series Runbook；请在原 Production Queue 中查看。</span></div>}
      {runbook.recommendationReason && <p className="production-batch-runbook-recommendation">推荐依据：{runbook.recommendationReason}</p>}
    </section>
  );
}

export function filterRunbookRows(rows: ProductionBatchRunbookRow[], filter: ProductionBatchRunbookFilter): ProductionBatchRunbookRow[] {
  if (filter === "all") return rows;
  if (filter === "active") return rows.filter((row) => row.batchStatus === "READY" || row.batchStatus === "RUNNING" || row.batchStatus === "PAUSED" || row.batchStatus === "COMPLETED");
  return rows.filter((row) => row.batchStatus.toLowerCase() === filter);
}

export function sortRunbookRows(rows: ProductionBatchRunbookRow[]): ProductionBatchRunbookRow[] {
  return [...rows].sort((left, right) => (left.seriesOrdinal ?? Number.MAX_SAFE_INTEGER) - (right.seriesOrdinal ?? Number.MAX_SAFE_INTEGER) || (left.episodeOrdinal ?? Number.MAX_SAFE_INTEGER) - (right.episodeOrdinal ?? Number.MAX_SAFE_INTEGER) || (left.sceneOrdinal ?? Number.MAX_SAFE_INTEGER) - (right.sceneOrdinal ?? Number.MAX_SAFE_INTEGER) || stagePriority(left.stage) - stagePriority(right.stage) || left.createdAt.localeCompare(right.createdAt) || left.batchId.localeCompare(right.batchId));
}

export function canStartRunbookRow(row: ProductionBatchRunbookRow, hasRunningBatch: boolean): boolean {
  return row.batchStatus === "READY" && row.readyToStart && !row.blockedReason && !row.mixedScope && !hasRunningBatch;
}

export function runbookProgress(row: ProductionBatchRunbookRow): number {
  if (row.shotCount <= 0) return 0;
  return Math.max(0, Math.min(100, Math.round((row.succeeded / row.shotCount) * 100)));
}

export function runbookFilterLabel(filter: ProductionBatchRunbookFilter): string {
  return { active: "当前与最近完成", ready: "READY", running: "RUNNING", paused: "PAUSED", completed: "COMPLETED", all: "全部" }[filter];
}

function stagePriority(stage: ProductionBatchRunbookRow["stage"]): number {
  return stage === "image" ? 0 : stage === "video" ? 1 : 2;
}

function runbookStatusLabel(status: ProductionBatchRunbookStatus): string {
  return { READY: "READY · 待启动", RUNNING: "RUNNING · 执行中", PAUSED: "PAUSED · 已暂停", COMPLETED: "COMPLETED · 已完成" }[status] ?? status;
}

function runbookStageLabel(stage: ProductionBatchRunbookRow["stage"]): string {
  return stage === "image" || stage === "video" ? sceneProductionStageLabel(stage) : "混合范围";
}

function ordinalLabel(value: number | null | undefined): string {
  return String((value ?? -1) + 1).padStart(2, "0");
}
