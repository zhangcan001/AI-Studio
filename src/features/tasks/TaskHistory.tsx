import { useCallback, useEffect, useState } from "react";
import { getTaskDetail, taskHistoryPage } from "../../services/tauriClient";
import { subscribeTaskUpdates } from "../../services/taskEvents";
import type { PageCursor } from "../../types/asset";
import type { ReusableGenerationDraft, TaskDetail, TaskHistoryFilter, TaskHistoryItem } from "../../types/history";
import type { TaskStatus } from "../../types/task";
import { AssetPreview } from "../assets/AssetPreview";
import { TaskHistoryDetail } from "./TaskHistoryDetail";
import { TaskHistoryList } from "./TaskHistoryList";

const PROJECT_ID = "prj_default";

interface Props {
  onLoadInputs: (draft: ReusableGenerationDraft) => void;
}

export function TaskHistory({ onLoadInputs }: Props) {
  const [filter, setFilter] = useState<TaskHistoryFilter>("ALL");
  const [items, setItems] = useState<TaskHistoryItem[]>([]);
  const [cursor, setCursor] = useState<PageCursor>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const [activityNotice, setActivityNotice] = useState(false);
  const [selectedTaskId, setSelectedTaskId] = useState<string>();
  const [detail, setDetail] = useState<TaskDetail>();
  const [previewAssetId, setPreviewAssetId] = useState<string>();

  const loadPage = useCallback(
    async (reset: boolean) => {
      setLoading(true);
      setError(undefined);
      try {
        const page = await taskHistoryPage(PROJECT_ID, filter, reset ? undefined : cursor, 30);
        setItems((current) =>
          reset
            ? page.items
            : [...current, ...page.items.filter((item) => !current.some((existing) => existing.id === item.id))],
        );
        setCursor(page.nextCursor);
        setActivityNotice(false);
      } catch (loadError: unknown) {
        setError(loadError instanceof Error ? loadError.message : String(loadError));
      } finally {
        setLoading(false);
      }
    },
    [cursor, filter],
  );

  useEffect(() => {
    setItems([]);
    setCursor(undefined);
    void loadPage(true);
  }, [filter]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void subscribeTaskUpdates((task) => {
      if (!active) return;
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
  }, []);

  async function selectTask(taskId: string) {
    setSelectedTaskId(taskId);
    setDetail(undefined);
    setError(undefined);
    try {
      setDetail(await getTaskDetail(PROJECT_ID, taskId));
    } catch (loadError: unknown) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    }
  }

  const previewAsset = detail?.outputAssets.find((asset) => asset.id === previewAssetId);

  return (
    <section className="workspace-panel">
      {selectedTaskId && detail ? (
        <TaskHistoryDetail
          detail={detail}
          loadingDraft={false}
          onBack={() => {
            setSelectedTaskId(undefined);
            setDetail(undefined);
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
      {previewAsset && <AssetPreview asset={previewAsset} onClose={() => setPreviewAssetId(undefined)} />}
    </section>
  );
}
