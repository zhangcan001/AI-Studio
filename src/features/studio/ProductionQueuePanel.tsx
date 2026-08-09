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

interface Props {
  projectId: string;
  batchItems: BatchDraftItem[];
  comfyConnected: boolean;
  onOpenTask: (taskId: string) => void;
}

export function ProductionQueuePanel({ projectId, batchItems, comfyConnected, onOpenTask }: Props) {
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
        setNotice(error instanceof Error ? error.message : String(error));
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
      refreshTimer = window.setTimeout(() => void refreshQueues(true), 900);
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
  }, [projectId, refreshQueues]);

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
      setNotice("Enter a production queue name.");
      return;
    }
    if (!batchItems.length) {
      setNotice("Add at least one item to the local batch first.");
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
      commitDetail(created, `Saved production queue with ${created.total} item${created.total === 1 ? "" : "s"}.`);
    } catch (error: unknown) {
      setNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function openQueue(batchId: string) {
    setBusy(true);
    setNotice(undefined);
    try {
      setQueueDetail(await getProductionQueue(projectId, batchId));
    } catch (error: unknown) {
      setNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function startQueue(batchId: string) {
    if (!comfyConnected) {
      setNotice("Connect ComfyUI before starting a production queue.");
      return;
    }
    await runMutation(async () => {
      const updated = await startProductionQueue(projectId, batchId);
      commitDetail(updated, "Production queue is running in the background.");
    });
  }

  async function pauseQueue(batchId: string) {
    await runMutation(async () => {
      const updated = await pauseProductionQueue(projectId, batchId);
      commitDetail(updated, "Queue paused. An already dispatched Task will be observed to its terminal state.");
    });
  }

  async function archiveQueue(batchId: string) {
    await runMutation(async () => {
      const updated = await archiveProductionQueue(projectId, batchId);
      commitDetail(updated, "Queue archived. Restore it before changing items or starting it again.");
    });
  }

  async function restoreQueue(batchId: string) {
    await runMutation(async () => {
      const updated = await restoreProductionQueue(projectId, batchId);
      commitDetail(updated, "Queue restored.");
    });
  }

  async function deleteQueue(batchId: string) {
    if (!window.confirm("Delete this archived production queue? Existing Tasks and Assets are kept.")) return;
    await runMutation(async () => {
      await deleteProductionQueue(projectId, batchId);
      setQueues((current) => current.filter((queue) => queue.id !== batchId));
      if (selectedIdRef.current === batchId) setQueueDetail(undefined);
      setNotice("Archived queue deleted. Existing Tasks and Assets were not deleted.");
      await refreshQueues(false);
    });
  }

  async function skipItem(itemId: string) {
    if (!detail) return;
    await runMutation(async () => {
      const updated = await skipProductionQueueItem(projectId, detail.id, itemId);
      commitDetail(updated, "Item marked as skipped. Resume the queue explicitly when ready.");
    });
  }

  async function requeueItem(itemId: string) {
    if (!detail) return;
    await runMutation(async () => {
      const updated = await requeueProductionQueueItem(projectId, detail.id, itemId);
      commitDetail(updated, "A new pending retry item was appended. The original failed Task remains unchanged.");
    });
  }

  async function runMutation(operation: () => Promise<void>) {
    setBusy(true);
    setNotice(undefined);
    try {
      await operation();
    } catch (error: unknown) {
      setNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  const detailEditable = Boolean(detail && !detail.archivedAt && detail.status !== "RUNNING");

  return (
    <section className="production-queue-panel" aria-label="Persistent production queues">
      <div className="production-queue-heading">
        <div>
          <span className="section-label">Persistent production queue</span>
          <p>Durable Kera2 + MiniMax H3 production orchestration. Task events refresh this view automatically.</p>
        </div>
        <button type="button" className="quiet-button" disabled={busy} onClick={() => void refreshQueues(true)}>
          Refresh
        </button>
      </div>

      {overview && (
        <div className="production-overview" aria-label="Production queue overview">
          <OverviewStat label="Queues" value={overview.totalQueues} />
          <OverviewStat label="Running" value={overview.runningQueues} />
          <OverviewStat label="Pending" value={overview.pendingItems} />
          <OverviewStat label="Active" value={overview.activeItems} />
          <OverviewStat label="Succeeded" value={overview.succeededItems} />
          <OverviewStat label="Failed" value={overview.failedItems} />
          <OverviewStat label="Archived" value={overview.archivedQueues} />
        </div>
      )}

      <div className="production-queue-create">
        <input
          aria-label="Production queue name"
          value={name}
          maxLength={120}
          onChange={(event) => setName(event.target.value)}
          placeholder="e.g. EP01 Kera2 + H3 production"
        />
        <label className="production-queue-checkbox">
          <input
            type="checkbox"
            checked={continueOnFailure}
            onChange={(event) => setContinueOnFailure(event.target.checked)}
          />
          Continue after non-execution failure/cancel
        </label>
        <button type="button" disabled={busy || !batchItems.length} onClick={() => void saveQueue()}>
          Save Queue ({batchItems.length})
        </button>
      </div>

      <div className="production-queue-filter">
        <label className="production-queue-checkbox">
          <input type="checkbox" checked={showArchived} onChange={(event) => setShowArchived(event.target.checked)} />
          Show archived ({overview?.archivedQueues ?? 0})
        </label>
        <span>{visibleQueues.length} visible queue{visibleQueues.length === 1 ? "" : "s"}</span>
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
                <span>{queue.archivedAt ? "ARCHIVED" : queue.status}</span>
              </button>
              <div className="production-queue-row-actions">
                {queue.archivedAt ? (
                  <>
                    <button type="button" className="quiet-button" onClick={() => void restoreQueue(queue.id)} disabled={busy}>
                      Restore
                    </button>
                    <button type="button" className="quiet-button danger-button" onClick={() => void deleteQueue(queue.id)} disabled={busy}>
                      Delete
                    </button>
                  </>
                ) : queue.status === "RUNNING" ? (
                  <button type="button" className="quiet-button" onClick={() => void pauseQueue(queue.id)} disabled={busy}>
                    Pause
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
                        {queue.status === "PAUSED" ? "Resume" : "Start"}
                      </button>
                    )}
                    <button type="button" className="quiet-button" onClick={() => void archiveQueue(queue.id)} disabled={busy}>
                      Archive
                    </button>
                  </>
                )}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <p className="disabled-note">No queues match the current filter.</p>
      )}

      {detail && (
        <div className="production-queue-detail">
          <div className="production-queue-detail-heading">
            <div>
              <span className="section-label">Selected queue</span>
              <strong>{detail.name}</strong>
              <span>{detail.archivedAt ? "ARCHIVED" : detail.status}</span>
            </div>
            <small>{detail.id}</small>
          </div>
          <div className="production-queue-stats">
            <span>Total <strong>{detail.total}</strong></span>
            <span>Pending <strong>{detail.pending}</strong></span>
            <span>Active <strong>{detail.running}</strong></span>
            <span>Succeeded <strong>{detail.succeeded}</strong></span>
            <span>Failed <strong>{detail.failed}</strong></span>
            <span>Cancelled <strong>{detail.cancelled}</strong></span>
            <span>Skipped <strong>{detail.skipped}</strong></span>
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
                  <strong>{item.status}</strong>
                  <div className="production-item-identity">
                    <span>{item.taskId ?? "Not dispatched"}</span>
                    {item.retryOfItemId && <small>Retry of {item.retryOfItemId}</small>}
                  </div>
                  <div className="production-item-error">
                    <span>{item.errorCode ?? "—"}</span>
                    {item.errorMessage && <small>{item.errorMessage}</small>}
                  </div>
                  <div className="production-item-actions">
                    {item.taskId && (
                      <button type="button" className="quiet-button" onClick={() => onOpenTask(item.taskId!)}>
                        Open Task
                      </button>
                    )}
                    {canSkip && (
                      <button type="button" className="quiet-button" onClick={() => void skipItem(item.id)} disabled={busy}>
                        Skip
                      </button>
                    )}
                    {canRequeue && (
                      <button type="button" className="quiet-button" onClick={() => void requeueItem(item.id)} disabled={busy}>
                        Requeue
                      </button>
                    )}
                    {reviewRequired && <span className="review-required">Review required</span>}
                  </div>
                </li>
              );
            })}
          </ol>
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
