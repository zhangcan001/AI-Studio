import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  archiveProductionQueue,
  cancelPendingProductionQueue,
  createProductionQueue,
  deleteProductionQueue,
  getProductionQueue,
  getProductionQueueOverview,
  deleteProductionQueueNamePreset,
  listProductionQueueNamePresets,
  listProductionQueues,
  pauseProductionQueue,
  requeueProductionQueueItem,
  restoreProductionQueue,
  skipProductionQueueItem,
  saveProductionQueueNamePreset,
  startProductionQueue,
  getReusableDraft,
  getTaskDetail,
} from "../../services/tauriClient";
import { subscribeTaskUpdates } from "../../services/taskEvents";
import type {
  ProductionBatchDetail,
  ProductionBatchSummary,
  ProductionQueueOverview,
} from "../../types/productionQueue";
import type { BatchDraftItem } from "./batchDraft";
import { canCancelPendingProductionQueue, isSafeProductionQueueRequeue } from "./productionQueuePolicy";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, productionItemStatusLabel, productionStatusLabel } from "../../i18n/statusLabels";
import type { ReusableGenerationDraft } from "../../types/history";
import type { AssetView } from "../../types/asset";
import { toggleCompareSelection } from "../assets/assetCompare";
import { AssetCompareWorkspace } from "../assets/AssetCompareWorkspace";
import { AssetCard } from "../assets/AssetCard";
import { ExperimentResultGrid } from "../experiments/ExperimentResultGrid";
import type { ExperimentContext } from "../experiments/experimentPlanner";
import { ProductionAssetPreview } from "./ProductionAssetPreview";
import { ProductionBatchReviewWorkspace } from "./ProductionBatchReviewWorkspace";

interface Props {
  projectId: string;
  batchItems: BatchDraftItem[];
  comfyConnected: boolean;
  focusBatchId?: string;
  onAdmissionChanged: () => Promise<void>;
  onFocusedBatchOpened: () => void;
  onOpenTask: (taskId: string) => void;
  hideCreate?: boolean;
  variant?: "full" | "inline";
  experimentContexts?: Record<string, ExperimentContext>;
  onPromoteWinner?: (draft: ReusableGenerationDraft, source: { batchName: string; taskId: string }) => Promise<void>;
}

export function ProductionQueuePanel({
  projectId,
  batchItems,
  comfyConnected,
  focusBatchId,
  onAdmissionChanged,
  onFocusedBatchOpened,
  onOpenTask,
  hideCreate = false,
  variant = "full",
  experimentContexts,
  onPromoteWinner,
}: Props) {
  const inline = variant === "inline";
  const [name, setName] = useState("");
  const [namePresets, setNamePresets] = useState<string[]>([]);
  const [selectedNamePreset, setSelectedNamePreset] = useState("");
  const [continueOnFailure, setContinueOnFailure] = useState(false);
  const [queues, setQueues] = useState<ProductionBatchSummary[]>([]);
  const [overview, setOverview] = useState<ProductionQueueOverview>();
  const [detail, setDetail] = useState<ProductionBatchDetail>();
  const [showArchived, setShowArchived] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string>();
  const [resultCompareAssets, setResultCompareAssets] = useState<AssetView[]>([]);
  const [resultCompareOpen, setResultCompareOpen] = useState(false);
  const [resultAssetsByItem, setResultAssetsByItem] = useState<Record<string, AssetView[]>>({});
  const [expandedInlineItemId, setExpandedInlineItemId] = useState<string>();
  const [previewAsset, setPreviewAsset] = useState<AssetView>();
  const selectedIdRef = useRef<string | undefined>(undefined);

  const setQueueDetail = useCallback((next?: ProductionBatchDetail) => {
    selectedIdRef.current = next?.id;
    setDetail(next);
  }, []);

  const refreshQueues = useCallback(
    async (refreshDetail = true) => {
      try {
        const [nextQueues, nextOverview] = await Promise.all([
          listProductionQueues(projectId),
          getProductionQueueOverview(projectId),
        ]);
        setQueues(nextQueues);
        setOverview(nextOverview);
        const selectedId = selectedIdRef.current;
        const autoFocusQueue = inline && !selectedId && !focusBatchId
          ? nextQueues.find((queue) => !queue.archivedAt && queue.status === "RUNNING")
            ?? nextQueues.find((queue) => !queue.archivedAt && queue.status === "PAUSED")
            ?? nextQueues.find((queue) => !queue.archivedAt && queue.status === "READY")
            ?? nextQueues.find((queue) => !queue.archivedAt)
          : undefined;
        const detailId = selectedId ?? autoFocusQueue?.id;
        if (detailId && (refreshDetail || Boolean(autoFocusQueue))) {
          try {
            setQueueDetail(await getProductionQueue(projectId, detailId));
          } catch {
            setQueueDetail(undefined);
          }
        }
      } catch (error: unknown) {
        setNotice(toUserMessage(error));
      }
    },
    [focusBatchId, inline, projectId, setQueueDetail],
  );

  useEffect(() => {
    setName("");
    setSelectedNamePreset("");
    setContinueOnFailure(false);
    setQueues([]);
    setOverview(undefined);
    setQueueDetail(undefined);
    setShowArchived(false);
    setNotice(undefined);
    setResultAssetsByItem({});
    setExpandedInlineItemId(undefined);
    void refreshQueues(false);
    void listProductionQueueNamePresets().then(setNamePresets).catch(() => setNamePresets([]));
  }, [projectId, refreshQueues, setQueueDetail]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    let refreshTimer: number | undefined;
    void subscribeTaskUpdates((task) => {
      if (!active || task.projectId !== projectId) return;
      if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
      // The queue runner observes the persisted terminal Task shortly after this event.
      refreshTimer = window.setTimeout(() => {
        void refreshQueues(true);
        void onAdmissionChanged();
      }, 900);
    })
      .then((cleanup) => {
        if (active) unlisten = cleanup;
        else cleanup();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
      unlisten?.();
    };
  }, [onAdmissionChanged, projectId, refreshQueues]);

  useEffect(() => {
    if (!focusBatchId) return;
    let active = true;
    setBusy(true);
    void getProductionQueue(projectId, focusBatchId)
      .then((focused) => {
        if (active) setQueueDetail(focused);
      })
      .catch((error: unknown) => {
        if (active) setNotice(toUserMessage(error));
      })
      .finally(() => {
        if (active) {
          setBusy(false);
          onFocusedBatchOpened();
        }
      });
    return () => {
      active = false;
    };
  }, [focusBatchId, onFocusedBatchOpened, projectId, setQueueDetail]);

  useEffect(() => {
    let active = true;
    const succeededItems = detail?.items.filter((item) => item.status === "SUCCEEDED" && item.taskId) ?? [];
    if (!succeededItems.length) return () => { active = false; };
    void Promise.all(
      succeededItems.map(async (item) => [
        item.id,
        (await getTaskDetail(projectId, item.taskId!)).outputAssets,
      ] as const),
    )
      .then((entries) => {
        if (!active) return;
        setResultAssetsByItem((current) => ({ ...current, ...Object.fromEntries(entries) }));
      })
      .catch(() => undefined);
    return () => { active = false; };
  }, [detail, projectId]);

  useEffect(() => {
    setExpandedInlineItemId(undefined);
  }, [detail?.id]);

  const visibleQueues = useMemo(
    () => queues.filter((queue) => showArchived || !queue.archivedAt),
    [queues, showArchived],
  );

  function mergeSummary(updated: ProductionBatchSummary) {
    setQueues((current) => [updated, ...current.filter((queue) => queue.id !== updated.id)]);
  }

  function commitDetail(updated: ProductionBatchDetail, message: string) {
    mergeSummary(updated);
    setQueueDetail(updated);
    setNotice(message);
    void refreshQueues(false);
  }

  async function saveQueue() {
    const trimmedName = name.trim();
    if (!trimmedName) {
      setNotice("请输入生产队列名称。");
      return;
    }
    if (!batchItems.length) {
      setNotice("请先在临时批量任务中添加至少一项。");
      return;
    }
    setBusy(true);
    setNotice(undefined);
    try {
      const created = await createProductionQueue({
        projectId,
        name: trimmedName,
        continueOnFailure,
        items: batchItems.map((item) => ({
          workflowVersionId: item.workflowVersionId,
          recipeId: item.recipeId,
          values: item.values,
        })),
      });
      setName("");
      commitDetail(created, `已保存生产队列，共 ${created.total} 项。`);
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      await onAdmissionChanged();
      setBusy(false);
    }
  }

  async function saveNamePreset() {
    if (!name.trim()) {
      setNotice("请输入队列名称后再保存模板。");
      return;
    }
    try {
      const next = await saveProductionQueueNamePreset(name);
      setNamePresets(next);
      setSelectedNamePreset(name.trim());
      setNotice("队列名称模板已保存到设置。");
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    }
  }

  async function removeNamePreset() {
    if (!selectedNamePreset) return;
    try {
      await deleteProductionQueueNamePreset(selectedNamePreset);
      setNamePresets((current) => current.filter((item) => item !== selectedNamePreset));
      setSelectedNamePreset("");
      setNotice("队列名称模板已删除。");
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    }
  }

  async function openQueue(batchId: string) {
    setBusy(true);
    setNotice(undefined);
    try {
      setQueueDetail(await getProductionQueue(projectId, batchId));
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function startQueue(batchId: string) {
    if (!comfyConnected) {
      setNotice("请先连接 ComfyUI，再开始生产队列。");
      return;
    }
    await runMutation(async () => {
      const updated = await startProductionQueue(projectId, batchId);
      commitDetail(updated, "生产队列已在后台运行。");
    });
  }

  async function pauseQueue(batchId: string) {
    await runMutation(async () => {
      const updated = await pauseProductionQueue(projectId, batchId);
      commitDetail(updated, "队列已暂停，已经提交的任务会继续运行到结束。");
    });
  }

  async function cancelPendingQueue(batchId: string) {
    if (!window.confirm("确定取消这个待开始的生产队列吗？尚未提交的项目会标记为已取消，不会启动 GPU 任务；已有任务和资产不受影响。")) return;
    await runMutation(async () => {
      const updated = await cancelPendingProductionQueue(projectId, batchId);
      commitDetail(updated, "队列已取消，未提交 GPU 任务。已有任务和资产未受影响。");
      try {
        await onAdmissionChanged();
      } catch {
        // The queue state is persisted; a status refresh can retry on the next update.
      }
    });
  }

  async function archiveQueue(batchId: string) {
    await runMutation(async () => {
      const updated = await archiveProductionQueue(projectId, batchId);
      commitDetail(updated, "队列已归档，如需修改或再次开始请先恢复队列。");
    });
  }

  async function restoreQueue(batchId: string) {
    await runMutation(async () => {
      const updated = await restoreProductionQueue(projectId, batchId);
      commitDetail(updated, "队列已恢复。");
    });
  }

  async function deleteQueue(batchId: string) {
    if (!window.confirm("确定删除这个已归档的生产队列吗？已有任务和资产会保留。")) return;
    await runMutation(async () => {
      await deleteProductionQueue(projectId, batchId);
      setQueues((current) => current.filter((queue) => queue.id !== batchId));
      if (selectedIdRef.current === batchId) setQueueDetail(undefined);
      setNotice("已删除归档队列，已有任务和资产未被删除。");
      await refreshQueues(false);
    });
  }

  async function skipItem(itemId: string) {
    if (!detail) return;
    await runMutation(async () => {
      const updated = await skipProductionQueueItem(projectId, detail.id, itemId);
      commitDetail(updated, "该项已标记为跳过，需要时请手动继续队列。");
    });
  }

  async function requeueItem(itemId: string) {
    if (!detail) return;
    await runMutation(async () => {
      const updated = await requeueProductionQueueItem(projectId, detail.id, itemId);
      commitDetail(updated, "已追加新的等待重试项，原失败任务保持不变。");
    });
  }

  async function useResultInStudio(item: ProductionBatchDetail["items"][number]) {
    if (!detail || !item.taskId || !onPromoteWinner) return;
    await runMutation(async () => {
      const draft = await getReusableDraft(projectId, item.taskId!);
      await onPromoteWinner(draft, { batchName: detail.name, taskId: item.taskId! });
    });
  }

  async function addResultToCompare(item: ProductionBatchDetail["items"][number]) {
    if (!item.taskId) return;
    await runMutation(async () => {
      const task = await getTaskDetail(projectId, item.taskId!);
      let next = resultCompareAssets;
      let message: string | undefined;
      for (const asset of task.outputAssets) {
        const result = toggleCompareSelection(next, asset);
        next = result.assets;
        message = result.notice ?? message;
      }
      setResultCompareAssets(next);
      if (next.length >= 2) setResultCompareOpen(true);
      setNotice(message ?? (next.length === 1 ? "已加入 1 个结果，继续选择同类型结果后可对比。" : "结果已加入对比。"));
    });
  }

  async function runMutation(operation: () => Promise<void>) {
    setBusy(true);
    setNotice(undefined);
    try {
      await operation();
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const detailEditable = Boolean(detail && !detail.archivedAt && detail.status !== "RUNNING");
  const processedCount = detail
    ? detail.succeeded + detail.failed + detail.cancelled + detail.skipped
    : 0;
  const progressPercent = detail && detail.total > 0
    ? Math.round((processedCount / detail.total) * 100)
    : 0;
  const activeItem = detail?.items.find((item) => item.status === "DISPATCHING" || item.status === "DISPATCHED");
  const canCancelPending = detail ? canCancelPendingProductionQueue(detail) : false;
  const renderedItems = detail?.items ?? [];

  return (
    <section className={`production-queue-panel${inline ? " production-queue-panel-inline" : ""}`} aria-label={inline ? "批次进度" : "生产队列"}>
      <div className="production-queue-heading">
        <div>
          <span className="section-label">{inline ? "批次进度" : "生产队列"}</span>
          <p>{inline ? "当前页实时同步，不需要切换到任务页。" : "管理持久化生产任务，任务事件会自动刷新此页面。"}</p>
        </div>
        <button type="button" className="quiet-button" disabled={busy} onClick={() => void refreshQueues(true)}>
          刷新
        </button>
      </div>

      {!inline && overview && (
        <div className="production-overview" aria-label="生产队列概览">
          <OverviewStat label="队列" value={overview.totalQueues} />
          <OverviewStat label="运行中" value={overview.runningQueues} />
          <OverviewStat label="等待中" value={overview.pendingItems} />
          <OverviewStat label="执行中" value={overview.activeItems} />
          <OverviewStat label="已完成" value={overview.succeededItems} />
          <OverviewStat label="失败" value={overview.failedItems} />
          <OverviewStat label="已归档" value={overview.archivedQueues} />
        </div>
      )}

      {!inline && !hideCreate && <div className="production-queue-create">
        <input
          aria-label="生产队列名称"
          value={name}
          maxLength={120}
          onChange={(event) => setName(event.target.value)}
          placeholder="例如：第 01 集 Kera2 + H3"
        />
        <div className="production-queue-name-presets">
          <label>
            <span>名称模板</span>
            <select value={selectedNamePreset} onChange={(event) => { const next = event.target.value; setSelectedNamePreset(next); if (next) setName(next); }} disabled={busy}>
              <option value="">选择模板</option>
              {namePresets.map((preset) => <option key={preset} value={preset}>{preset}</option>)}
            </select>
          </label>
          <button type="button" className="quiet-button" onClick={() => void saveNamePreset()} disabled={busy || !name.trim()}>保存模板</button>
          <button type="button" className="quiet-button" onClick={() => void removeNamePreset()} disabled={busy || !selectedNamePreset}>删除模板</button>
        </div>
        <label className="production-queue-checkbox">
          <input
            type="checkbox"
            checked={continueOnFailure}
            onChange={(event) => setContinueOnFailure(event.target.checked)}
          />
          非执行失败或取消后继续
        </label>
        <button type="button" disabled={busy || !batchItems.length} onClick={() => void saveQueue()}>
          保存队列（{batchItems.length}）
        </button>
      </div>}

      {!inline && <div className="production-queue-filter">
        <label className="production-queue-checkbox">
          <input type="checkbox" checked={showArchived} onChange={(event) => setShowArchived(event.target.checked)} />
          显示已归档（{overview?.archivedQueues ?? 0}）
        </label>
        <span>显示 {visibleQueues.length} 个队列</span>
      </div>}

      {!inline && (visibleQueues.length ? (
        <div className="production-queue-list">
          {visibleQueues.slice(0, 12).map((queue) => (
            <div key={queue.id} className={`production-queue-row${queue.archivedAt ? " archived" : ""}`}>
              <button
                type="button"
                className="production-queue-open"
                onClick={() => void openQueue(queue.id)}
                disabled={busy}
              >
                <strong>{queue.name}</strong>
                <span>{queue.archivedAt ? "已归档" : productionStatusLabel(queue.status)}</span>
              </button>
              <div className="production-queue-row-actions">
                {queue.archivedAt ? (
                  <>
                    <button type="button" className="quiet-button" onClick={() => void restoreQueue(queue.id)} disabled={busy}>
                      恢复
                    </button>
                    <button type="button" className="quiet-button danger-button" onClick={() => void deleteQueue(queue.id)} disabled={busy}>
                      删除
                    </button>
                  </>
                ) : queue.status === "RUNNING" ? (
                  <button type="button" className="quiet-button" onClick={() => void pauseQueue(queue.id)} disabled={busy}>
                    暂停
                  </button>
                ) : (
                  <>
                    {queue.status !== "COMPLETED" && (
                      <button
                        type="button"
                        className="quiet-button"
                        onClick={() => void startQueue(queue.id)}
                        disabled={busy || !comfyConnected}
                      >
                        {queue.status === "PAUSED" ? "继续" : "开始"}
                      </button>
                    )}
                    <button type="button" className="quiet-button" onClick={() => void archiveQueue(queue.id)} disabled={busy}>
                      归档
                    </button>
                  </>
                )}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <p className="disabled-note">当前筛选条件下没有找到队列。</p>
      ))}

      {inline && !detail && (
        <div className="production-inline-empty" role="status">
          <strong>创建批次后，进度会显示在这里</strong>
          <span>任务会按顺序执行，完成、失败和等待状态都会自动更新。</span>
        </div>
      )}

      {detail && (
        <div className="production-queue-detail">
          {inline && (
            <div className="production-inline-progress" role="status" aria-live="polite">
              <div className="production-inline-progress-heading">
                <div>
                  <strong>{processedCount} / {detail.total} 已处理</strong>
                  <span>
                    {activeItem
                      ? `正在执行第 ${activeItem.ordinal + 1} 项`
                      : detail.status === "COMPLETED"
                        ? "批次已完成"
                        : productionStatusLabel(detail.status)}
                  </span>
                </div>
                <strong>{progressPercent}%</strong>
              </div>
              <div
                className="production-inline-progress-track"
                role="progressbar"
                aria-label="批次完成进度"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={progressPercent}
              >
                <span style={{ width: `${progressPercent}%` }} />
              </div>
            </div>
          )}
          <div className="production-queue-detail-heading">
            <div>
              <span className="section-label">{inline ? "当前批次" : "当前队列"}</span>
              <strong>{detail.name}</strong>
              <span>{detail.archivedAt ? "已归档" : productionStatusLabel(detail.status)}</span>
            </div>
            <div className="production-queue-detail-heading-actions">
              {canCancelPending && (
                <button
                  type="button"
                  className="quiet-button danger-button"
                  onClick={() => void cancelPendingQueue(detail.id)}
                  disabled={busy}
                  title="取消所有尚未提交的队列项目，不会启动 GPU 任务"
                >
                  取消待开始
                </button>
              )}
              <small>{detail.id}</small>
            </div>
          </div>
          <div className="production-queue-stats">
            <span>总数 <strong>{detail.total}</strong></span>
            <span>等待中 <strong>{detail.pending}</strong></span>
            <span>执行中 <strong>{detail.running}</strong></span>
            <span>已完成 <strong>{detail.succeeded}</strong></span>
            <span>失败 <strong>{detail.failed}</strong></span>
            <span>已取消 <strong>{detail.cancelled}</strong></span>
            <span>已跳过 <strong>{detail.skipped}</strong></span>
          </div>
          {inline && <div className="production-inline-queue-label">
            <span className="section-label">项目列表</span>
            <span>点击一项查看生成图和操作</span>
          </div>}
          <ol className={`production-queue-items${inline ? " production-inline-queue-list" : ""}`} aria-label={inline ? "批次项目列表" : undefined}>
            {renderedItems.map((item) => {
              const canSkip = detailEditable && (item.status === "FAILED" || item.status === "CANCELLED");
              const canRequeue = detailEditable && isSafeProductionQueueRequeue(item);
              const reviewRequired =
                detailEditable && item.status === "FAILED" && !canRequeue && item.errorCode !== undefined;
              const resultAssets = resultAssetsByItem[item.id] ?? [];
              const expanded = expandedInlineItemId === item.id;
              const itemLabel = item.promptText?.trim() || item.taskId || `第 ${item.ordinal + 1} 项`;
              const itemBody = (
                <>
                  <span>#{item.ordinal + 1}</span>
                  <strong>{productionItemStatusLabel(item.status)}</strong>
                  <div className="production-item-identity">
                    <span>{item.taskId ?? "尚未提交"}</span>
                    {item.retryOfItemId && <small>重试自 {item.retryOfItemId}</small>}
                    {item.promptText && <small title={item.promptText}>提示词：{item.promptText}</small>}
                    {item.seed && <small>Seed：{item.seed}</small>}
                    {item.createdAt && <small>{formatDateTime(item.createdAt)}</small>}
                  </div>
                  <div className="production-item-error">
                    <span>{item.errorCode ?? "—"}</span>
                    {item.errorMessage && <small>{toUserMessage({ code: item.errorCode, message: item.errorMessage })}</small>}
                  </div>
                  <div className="production-item-actions">
                    {item.taskId && (
                      <>
                        <button type="button" className="quiet-button" onClick={() => onOpenTask(item.taskId!)}>
                          打开任务
                        </button>
                        {item.status === "SUCCEEDED" && <>
                          <button type="button" className="quiet-button" onClick={() => onOpenTask(item.taskId!)}>查看结果</button>
                          {onPromoteWinner && <button type="button" className="quiet-button" onClick={() => void useResultInStudio(item)} disabled={busy}>用于创作</button>}
                          <button type="button" className="quiet-button" onClick={() => void addResultToCompare(item)} disabled={busy}>加入对比</button>
                        </>}
                      </>
                    )}
                    {canSkip && (
                      <button type="button" className="quiet-button" onClick={() => void skipItem(item.id)} disabled={busy}>
                        跳过
                      </button>
                    )}
                    {canRequeue && (
                      <button type="button" className="quiet-button" onClick={() => void requeueItem(item.id)} disabled={busy}>
                        重新排队
                      </button>
                    )}
                    {reviewRequired && <span className="review-required">需要检查</span>}
                  </div>
                  {resultAssets.length > 0 && item.taskId && (
                    <div className="production-item-results" aria-label={`第 ${item.ordinal + 1} 项输出资产`}>
                      {resultAssets.map((asset) => (
                        <AssetCard
                          key={asset.id}
                          projectId={projectId}
                          asset={asset}
                          onSelect={(selectedAsset) => setPreviewAsset(selectedAsset)}
                        />
                      ))}
                    </div>
                  )}
                </>
              );
              return (
                <li key={item.id} className={inline ? "production-inline-queue-item" : undefined}>
                  {inline ? (
                    <>
                      <button
                        type="button"
                        className={`production-inline-item-toggle${expanded ? " is-expanded" : ""}`}
                        aria-expanded={expanded}
                        aria-label={`${expanded ? "收起" : "查看"}第 ${item.ordinal + 1} 项${item.status === "SUCCEEDED" ? "生成图" : "详情"}`}
                        onClick={() => setExpandedInlineItemId((current) => current === item.id ? undefined : item.id)}
                      >
                        <span className="production-inline-item-index">#{item.ordinal + 1}</span>
                        <strong className="production-inline-item-status">{productionItemStatusLabel(item.status)}</strong>
                        <span className="production-inline-item-copy">
                          <strong title={itemLabel}>{itemLabel}</strong>
                          <small>{item.taskId ?? "尚未提交"}</small>
                        </span>
                        <span className="production-inline-item-result">
                          {item.status === "SUCCEEDED" ? (resultAssets.length ? `${resultAssets.length} 张图 · 打开全图` : "查看生成图") : item.errorCode ?? productionItemStatusLabel(item.status)}
                        </span>
                        <span className="production-inline-item-chevron" aria-hidden="true">{expanded ? "收起" : "查看"}</span>
                      </button>
                      {expanded && <div className="production-inline-item-detail">{itemBody}</div>}
                    </>
                  ) : itemBody}
                </li>
              );
            })}
          </ol>
          {detail.name.startsWith("实验 ·") && onPromoteWinner && (
            <ExperimentResultGrid
              projectId={projectId}
              batch={detail}
              recipe={experimentContexts?.[detail.id]?.recipe}
              baseValues={experimentContexts?.[detail.id]?.baseValues}
              onPromoteWinner={onPromoteWinner}
            />
          )}
          {detail.items.some((item) => {
            const identity = `${item.workflowVersionId} ${item.recipeId}`.toLowerCase();
            return identity.includes("minimax") || identity.includes("h3");
          }) && (
            <ProductionBatchReviewWorkspace
              projectId={projectId}
              batchId={detail.id}
              refreshKey={detail.items.map((item) => `${item.id}:${item.status}:${item.taskId ?? ""}:${item.updatedAt ?? ""}`).join("|")}
              onOpenTask={onOpenTask}
              onBatchChanged={async () => {
                await refreshQueues(true);
                await onAdmissionChanged();
              }}
            />
          )}
        </div>
      )}
      {notice && <p className="disabled-note">{notice}</p>}
      {resultCompareOpen && resultCompareAssets.length >= 2 && (
        <AssetCompareWorkspace
          projectId={projectId}
          assets={resultCompareAssets}
          onRemove={(assetId) => setResultCompareAssets((current) => current.filter((asset) => asset.id !== assetId))}
          onClear={() => { setResultCompareAssets([]); setResultCompareOpen(false); }}
          onClose={() => setResultCompareOpen(false)}
        />
      )}
      {previewAsset && (
        <ProductionAssetPreview
          projectId={projectId}
          asset={previewAsset}
          onClose={() => setPreviewAsset(undefined)}
          onOpenTask={onOpenTask}
        />
      )}
    </section>
  );
}

function OverviewStat({ label, value }: { label: string; value: number }) {
  return (
    <span>
      {label} <strong>{value}</strong>
    </span>
  );
}
