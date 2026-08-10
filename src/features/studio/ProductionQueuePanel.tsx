import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  archiveProductionQueue,
  createProductionQueue,
  deleteProductionQueue,
  getProductionQueue,
  getProductionQueueOverview,
  listProductionQueues,
  pauseProductionQueue,
  requeueProductionQueueItem,
  restoreProductionQueue,
  skipProductionQueueItem,
  startProductionQueue,
} from "../../services/tauriClient";
import { subscribeTaskUpdates } from "../../services/taskEvents";
import type {
  ProductionBatchDetail,
  ProductionBatchSummary,
  ProductionQueueOverview,
} from "../../types/productionQueue";
import type { BatchDraftItem } from "./batchDraft";
import { isSafeProductionQueueRequeue } from "./productionQueuePolicy";
import { toUserMessage } from "../../i18n/errorMessages";
import { productionItemStatusLabel, productionStatusLabel } from "../../i18n/statusLabels";
import type { ReusableGenerationDraft } from "../../types/history";
import { ExperimentResultGrid } from "../experiments/ExperimentResultGrid";
import type { ExperimentContext } from "../experiments/experimentPlanner";

interface Props {
  projectId: string;
  batchItems: BatchDraftItem[];
  comfyConnected: boolean;
  focusBatchId?: string;
  onAdmissionChanged: () => Promise<void>;
  onFocusedBatchOpened: () => void;
  onOpenTask: (taskId: string) => void;
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
  experimentContexts,
  onPromoteWinner,
}: Props) {
  const [name, setName] = useState("");
  const [continueOnFailure, setContinueOnFailure] = useState(false);
  const [queues, setQueues] = useState<ProductionBatchSummary[]>([]);
  const [overview, setOverview] = useState<ProductionQueueOverview>();
  const [detail, setDetail] = useState<ProductionBatchDetail>();
  const [showArchived, setShowArchived] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string>();
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
        if (refreshDetail && selectedId) {
          try {
            setQueueDetail(await getProductionQueue(projectId, selectedId));
          } catch {
            setQueueDetail(undefined);
          }
        }
      } catch (error: unknown) {
        setNotice(toUserMessage(error));
      }
    },
    [projectId, setQueueDetail],
  );

  useEffect(() => {
    setName("");
    setContinueOnFailure(false);
    setQueues([]);
    setOverview(undefined);
    setQueueDetail(undefined);
    setShowArchived(false);
    setNotice(undefined);
    void refreshQueues(false);
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

  return (
    <section className="production-queue-panel" aria-label="生产队列">
      <div className="production-queue-heading">
        <div>
          <span className="section-label">生产队列</span>
          <p>管理持久化生产任务，任务事件会自动刷新此页面。</p>
        </div>
        <button type="button" className="quiet-button" disabled={busy} onClick={() => void refreshQueues(true)}>
          刷新
        </button>
      </div>

      {overview && (
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

      <div className="production-queue-create">
        <input
          aria-label="生产队列名称"
          value={name}
          maxLength={120}
          onChange={(event) => setName(event.target.value)}
          placeholder="例如：第 01 集 Kera2 + H3"
        />
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
      </div>

      <div className="production-queue-filter">
        <label className="production-queue-checkbox">
          <input type="checkbox" checked={showArchived} onChange={(event) => setShowArchived(event.target.checked)} />
          显示已归档（{overview?.archivedQueues ?? 0}）
        </label>
        <span>显示 {visibleQueues.length} 个队列</span>
      </div>

      {visibleQueues.length ? (
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
      )}

      {detail && (
        <div className="production-queue-detail">
          <div className="production-queue-detail-heading">
            <div>
              <span className="section-label">当前队列</span>
              <strong>{detail.name}</strong>
              <span>{detail.archivedAt ? "已归档" : productionStatusLabel(detail.status)}</span>
            </div>
            <small>{detail.id}</small>
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
          <ol className="production-queue-items">
            {detail.items.map((item) => {
              const canSkip = detailEditable && (item.status === "FAILED" || item.status === "CANCELLED");
              const canRequeue = detailEditable && isSafeProductionQueueRequeue(item);
              const reviewRequired =
                detailEditable && item.status === "FAILED" && !canRequeue && item.errorCode !== undefined;
              return (
                <li key={item.id}>
                  <span>#{item.ordinal + 1}</span>
                  <strong>{productionItemStatusLabel(item.status)}</strong>
                  <div className="production-item-identity">
                    <span>{item.taskId ?? "尚未提交"}</span>
                    {item.retryOfItemId && <small>重试自 {item.retryOfItemId}</small>}
                  </div>
                  <div className="production-item-error">
                    <span>{item.errorCode ?? "—"}</span>
                    {item.errorMessage && <small>{toUserMessage({ code: item.errorCode, message: item.errorMessage })}</small>}
                  </div>
                  <div className="production-item-actions">
                    {item.taskId && (
                      <button type="button" className="quiet-button" onClick={() => onOpenTask(item.taskId!)}>
                        查看任务
                      </button>
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
        </div>
      )}
      {notice && <p className="disabled-note">{notice}</p>}
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
