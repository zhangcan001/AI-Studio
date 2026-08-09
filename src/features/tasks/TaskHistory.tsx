import { useCallback, useEffect, useRef, useState } from "react";
import { getTaskDetail, taskHistoryPage } from "../../services/tauriClient";
import { subscribeTaskUpdates } from "../../services/taskEvents";
import type { PageCursor } from "../../types/asset";
import type { ReusableGenerationDraft, TaskDetail, TaskHistoryFilter, TaskHistoryItem } from "../../types/history";
import type { TaskStatus } from "../../types/task";
import { AssetPreview } from "../assets/AssetPreview";
import { TaskHistoryDetail } from "./TaskHistoryDetail";
import { TaskHistoryList } from "./TaskHistoryList";

interface Props {
  projectId: string;
  comfyConnected: boolean;
  productionBusy: boolean;
  focusTaskId?: string;
  onLoadInputs: (draft: ReusableGenerationDraft) => void;
}

export function TaskHistory({ projectId, comfyConnected, productionBusy, focusTaskId, onLoadInputs }: Props) {
  const [filter, setFilter] = useState<TaskHistoryFilter>("ALL");
  const [items, setItems] = useState<TaskHistoryItem[]>([]);
  const [cursor, setCursor] = useState<PageCursor>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const [activityNotice, setActivityNotice] = useState(false);
  const [selectedTaskId, setSelectedTaskId] = useState<string>();
  const [detail, setDetail] = useState<TaskDetail>();
  const [previewAssetId, setPreviewAssetId] = useState<string>();
  const requestVersion = useRef(0);
  const detailVersion = useRef(0);

  const loadPage = useCallback(
    async (reset: boolean) => {
      const version = ++requestVersion.current;
      const requestedCursor = reset ? undefined : cursor;
      setLoading(true);
      setError(undefined);
      try {
        const page = await taskHistoryPage(projectId, filter, requestedCursor, 30);
        if (requestVersion.current !== version) return;
        setItems((current) =>
          reset
            ? page.items
            : [...current, ...page.items.filter((item) => !current.some((existing) => existing.id === item.id))],
        );
        setCursor(page.nextCursor);
        setActivityNotice(false);
      } catch (loadError: unknown) {
        if (requestVersion.current === version) {
          setError(loadError instanceof Error ? loadError.message : String(loadError));
        }
      } finally {
        if (requestVersion.current === version) setLoading(false);
      }
    },
    [cursor, filter, projectId],
  );

  useEffect(() => {
    detailVersion.current += 1;
    setItems([]);
    setCursor(undefined);
    setSelectedTaskId(undefined);
    setDetail(undefined);
    setPreviewAssetId(undefined);
    setActivityNotice(false);
    void loadPage(true);
  }, [filter, projectId]); // loadPage intentionally captures the reset context for this effect.

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void subscribeTaskUpdates((task) => {
      if (!active || task.projectId !== projectId) return;
      setItems((current) => {
        const index = current.findIndex((item) => item.id === task.id);
        if (index < 0) {
          setActivityNotice(true);
          return current;
        }
        const updated = [...current];
        updated[index] = {
          ...updated[index],
          status: task.status as TaskStatus,
          queuedAt: task.queuedAt,
          startedAt: task.startedAt,
          finishedAt: task.finishedAt,
          errorCode: task.error?.code,
          outputCount: task.outputAssetIds.length || updated[index].outputCount,
        };
        return updated;
      });
    })
      .then((cleanup) => {
        if (active) unlisten = cleanup;
        else cleanup();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      unlisten?.();
    };
  }, [projectId]);

  async function selectTask(taskId: string) {
    const version = ++detailVersion.current;
    setSelectedTaskId(taskId);
    setDetail(undefined);
    setPreviewAssetId(undefined);
    setError(undefined);
    try {
      const nextDetail = await getTaskDetail(projectId, taskId);
      if (detailVersion.current === version) setDetail(nextDetail);
    } catch (loadError: unknown) {
      if (detailVersion.current === version) {
        setError(loadError instanceof Error ? loadError.message : String(loadError));
      }
    }
  }

  useEffect(() => {
    if (focusTaskId) void selectTask(focusTaskId);
  }, [focusTaskId, projectId]); // The focused id is an explicit navigation request; selectTask reads current project state.

  const previewAsset = detail?.outputAssets.find((asset) => asset.id === previewAssetId);

  return (
    <section className="workspace-panel" aria-busy={loading}>
      {selectedTaskId && detail ? (
        <TaskHistoryDetail
          projectId={projectId}
          detail={detail}
          loadingDraft={false}
          comfyConnected={comfyConnected}
          productionBusy={productionBusy}
          onBack={() => {
            setSelectedTaskId(undefined);
            setDetail(undefined);
            setPreviewAssetId(undefined);
          }}
          onLoadInputs={onLoadInputs}
          onOpenAsset={setPreviewAssetId}
        />
      ) : (
        <>
          <TaskHistoryList
            filter={filter}
            items={items}
            nextCursor={cursor}
            loading={loading}
            onFilterChange={setFilter}
            onSelect={(taskId) => void selectTask(taskId)}
            onRefresh={() => void loadPage(true)}
            onLoadMore={() => void loadPage(false)}
          />
          {activityNotice && (
            <div className="activity-notice" role="status">
              New task activity is available. <button type="button" onClick={() => void loadPage(true)}>Refresh history</button>
            </div>
          )}
        </>
      )}
      {error && <p className="error-message">Unable to load task history: {error}</p>}
      {previewAsset && (
        <AssetPreview projectId={projectId} asset={previewAsset} onClose={() => setPreviewAssetId(undefined)} />
      )}
    </section>
  );
}
