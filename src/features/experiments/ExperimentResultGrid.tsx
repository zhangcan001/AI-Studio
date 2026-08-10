import { useEffect, useMemo, useState } from "react";
import {
  getReusableDraft,
  getTaskDetail,
} from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import type { ReusableGenerationDraft, TaskDetail } from "../../types/history";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import { taskStatusLabel } from "../../i18n/statusLabels";
import { toUserMessage } from "../../i18n/errorMessages";
import { AssetCompareWorkspace } from "../assets/AssetCompareWorkspace";
import { toggleCompareSelection } from "../assets/assetCompare";
import {
  displayVariantValue,
  experimentTaskDurationMs,
  snapshotDiff,
} from "./experimentPlanner";

interface Props {
  projectId: string;
  batch: ProductionBatchDetail;
  recipe?: RecipeViewModel;
  baseValues?: GenerationValues;
  onPromoteWinner: (draft: ReusableGenerationDraft, source: { batchName: string; taskId: string }) => Promise<void>;
}

interface ResultRecord {
  itemId: string;
  task?: TaskDetail;
  draft?: ReusableGenerationDraft;
  loading: boolean;
  error?: string;
}

export function ExperimentResultGrid({ projectId, batch, recipe, baseValues, onPromoteWinner }: Props) {
  const [records, setRecords] = useState<ResultRecord[]>([]);
  const [compareAssets, setCompareAssets] = useState<AssetView[]>([]);
  const [compareOpen, setCompareOpen] = useState(false);
  const [notice, setNotice] = useState<string>();
  const fieldLabels = useMemo(
    () => Object.fromEntries((recipe?.fields ?? []).map((field) => [field.key, field.label])),
    [recipe],
  );

  useEffect(() => {
    let active = true;
    const initial = batch.items.map((item) => ({ itemId: item.id, loading: Boolean(item.taskId) }));
    setRecords(initial);
    void Promise.all(batch.items.map(async (item) => {
      if (!item.taskId) return { itemId: item.id, loading: false } satisfies ResultRecord;
      try {
        const task = await getTaskDetail(projectId, item.taskId);
        let draft: ReusableGenerationDraft | undefined;
        if (task.reusableDraft.available) {
          try {
            draft = await getReusableDraft(projectId, task.id);
          } catch {
            // A terminal task may be visible before its reusable snapshot is queryable.
          }
        }
        return { itemId: item.id, task, draft, loading: false } satisfies ResultRecord;
      } catch (error: unknown) {
        return { itemId: item.id, loading: false, error: toUserMessage(error) } satisfies ResultRecord;
      }
    })).then((next) => {
      if (active) setRecords(next);
    });
    return () => {
      active = false;
    };
  }, [batch.items, projectId]);

  const firstDraft = baseValues ?? records.find((record) => record.draft)?.draft?.values;

  function addToCompare(asset: AssetView) {
    const result = toggleCompareSelection(compareAssets, asset);
    setCompareAssets(result.assets);
    setNotice(result.notice);
  }

  return (
    <section className="experiment-result-grid" aria-label="实验结果">
      <div className="experiment-result-heading">
        <div>
          <span className="section-label">实验结果</span>
          <h3>结果择优</h3>
          <p>结果仍来自普通 Task、Snapshot 和 Asset；这里只显示摘要差异，不暴露完整快照。</p>
        </div>
        <div className="experiment-compare-toolbar">
          <span>已选 {compareAssets.length} / 4 个结果</span>
          <button type="button" onClick={() => setCompareOpen(true)} disabled={compareAssets.length < 2}>打开对比</button>
          <button type="button" className="quiet-button" onClick={() => setCompareAssets([])} disabled={!compareAssets.length}>清空</button>
        </div>
      </div>
      <div className="experiment-result-cards">
        {batch.items.map((item) => {
          const record = records.find((candidate) => candidate.itemId === item.id);
          const diffBase = firstDraft;
          const diff = record?.draft && diffBase ? snapshotDiff(diffBase, record.draft.values, fieldLabels) : [];
          const duration = experimentTaskDurationMs(record?.task?.createdAt, record?.task?.finishedAt, record?.task?.startedAt);
          const seed = record?.draft?.values && Object.values(record.draft.values).find((value) => value.type === "seed_fixed");
          return (
            <article className="experiment-result-card" key={item.id}>
              <div className="experiment-result-card-heading">
                <strong>#{item.ordinal + 1}</strong>
                <span className={`status-pill task-${(record?.task?.status ?? item.status).toLowerCase()}`}>{record?.task ? taskStatusLabel(record.task.status) : item.status}</span>
              </div>
              <dl className="experiment-result-facts">
                <div><dt>变化字段</dt><dd>{diff.length ? diff.map((entry) => `${entry.fieldKey}：${entry.after}`).join(" · ") : "基准或等待快照"}</dd></div>
                <div><dt>Seed</dt><dd>{seed ? displayVariantValue(seed) : "—"}</dd></div>
                <div><dt>任务用时</dt><dd>{duration === undefined ? "—" : formatDuration(duration)}</dd></div>
              </dl>
              {record?.loading && <p className="disabled-note">正在加载结果...</p>}
              {record?.error && <p className="error-message">结果加载失败：{record.error}</p>}
              {record?.task?.outputAssets.length ? (
                <div className="experiment-result-assets">
                  {record.task.outputAssets.map((asset) => (
                    <button type="button" className="quiet-button" key={asset.id} onClick={() => addToCompare(asset)}>
                      加入对比 · {asset.name}
                    </button>
                  ))}
                </div>
              ) : (
                <p className="disabled-note">暂无输出资产。</p>
              )}
              <button
                type="button"
                className="experiment-promote-button"
                disabled={!record?.draft || Boolean(record.draft.missingAssetIds.length)}
                onClick={() => record?.draft && record.task && void onPromoteWinner(record.draft, { batchName: batch.name, taskId: record.task.id })}
              >
                作为下一轮起点
              </button>
              {record?.draft?.missingAssetIds.length ? <small className="disabled-note">缺少素材，加载后需要重新选择。</small> : null}
            </article>
          );
        })}
      </div>
      {notice && <p className="disabled-note" role="status">{notice}</p>}
      {compareOpen && compareAssets.length >= 2 && (
        <AssetCompareWorkspace
          projectId={projectId}
          assets={compareAssets}
          onRemove={(assetId) => setCompareAssets((current) => current.filter((asset) => asset.id !== assetId))}
          onClear={() => setCompareAssets([])}
          onClose={() => setCompareOpen(false)}
        />
      )}
    </section>
  );
}

function formatDuration(durationMs: number): string {
  const seconds = Math.round(durationMs / 1000);
  if (seconds < 60) return `${seconds} 秒`;
  return `${Math.floor(seconds / 60)} 分 ${seconds % 60} 秒`;
}
