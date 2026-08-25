import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getProductionQueueOverview,
  listProductionQueues,
  listPromptLibrary,
  pauseProductionQueue,
  startProductionQueue,
  taskHistoryPage,
} from "../../services/tauriClient";
import type { PromptEntryView } from "../../types/prompt";
import type { ProductionBatchSummary, ProductionQueueOverview } from "../../types/productionQueue";
import type { RecipeViewModel } from "../../types/generation";
import type { TaskHistoryItem } from "../../types/history";
import { toUserMessage } from "../../i18n/errorMessages";
import { fieldLabel, formatDateTime, productionStatusLabel, workflowDisplayName } from "../../i18n/statusLabels";
import {
  productionQueueAction,
  recentPromptEntries,
  recentProductionQueues,
  recentWorkflowRecords,
  summarizeProductionQueues,
  type RecentWorkflowRecord,
} from "./productionUx";

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  selectedWorkflow?: RecipeViewModel;
  promptTargetFieldKey: string;
  onPromptTargetFieldChange: (fieldKey: string) => void;
  onUsePrompt: (entry: PromptEntryView, fieldKey: string) => void;
  onContinueWorkflow: (record: RecentWorkflowRecord, recipe?: RecipeViewModel) => void;
  onFocusQueue: (batchId: string) => void;
  onAdmissionChanged: () => Promise<void>;
}

export function CreationDashboard({
  projectId,
  catalog,
  selectedWorkflow,
  promptTargetFieldKey,
  onPromptTargetFieldChange,
  onUsePrompt,
  onContinueWorkflow,
  onFocusQueue,
  onAdmissionChanged,
}: Props) {
  const [queues, setQueues] = useState<ProductionBatchSummary[]>([]);
  const [overview, setOverview] = useState<ProductionQueueOverview>();
  const [history, setHistory] = useState<TaskHistoryItem[]>([]);
  const [prompts, setPrompts] = useState<PromptEntryView[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyQueueId, setBusyQueueId] = useState<string>();
  const [error, setError] = useState<string>();

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(undefined);
    try {
      const [nextQueues, nextOverview, historyPage, promptPage] = await Promise.all([
        listProductionQueues(projectId),
        getProductionQueueOverview(projectId),
        taskHistoryPage({ projectId, filter: "SUCCEEDED", timeFilter: "ALL", limit: 50 }),
        listPromptLibrary(projectId, { limit: 30 }),
      ]);
      setQueues(nextQueues);
      setOverview(nextOverview);
      setHistory(historyPage.items);
      setPrompts(promptPage.items);
    } catch (value: unknown) {
      setError(toUserMessage(value));
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => { void refresh(); }, [refresh]);

  const summary = useMemo(() => summarizeProductionQueues(queues, overview), [overview, queues]);
  const recentQueues = useMemo(() => recentProductionQueues(queues, 5), [queues]);
  const recentWorkflows = useMemo(() => recentWorkflowRecords(history.map((item) => ({
    workflowVersionId: item.workflowVersionId,
    recipeId: item.recipeId,
    workflowName: item.workflowName,
    lastUsedAt: item.finishedAt ?? item.createdAt,
  })), 5), [history]);
  const recentPrompts = useMemo(() => recentPromptEntries(prompts, 5), [prompts]);
  const textFields = selectedWorkflow?.fields.filter((field) => field.type === "textarea") ?? [];

  async function actOnQueue(queue: ProductionBatchSummary) {
    const action = productionQueueAction(queue.status, queue.archivedAt);
    if (action === "查看") {
      onFocusQueue(queue.id);
      return;
    }
    setBusyQueueId(queue.id);
    setError(undefined);
    try {
      if (queue.status === "RUNNING") await pauseProductionQueue(projectId, queue.id);
      else await startProductionQueue(projectId, queue.id);
      await refresh();
      await onAdmissionChanged();
    } catch (value: unknown) {
      setError(toUserMessage(value));
    } finally {
      setBusyQueueId(undefined);
    }
  }

  return (
    <details className="creation-dashboard">
      <summary><span><span className="section-label">生产面板</span><strong>生产概览</strong></span><small>最近使用 · 队列状态 · 不自动生成</small></summary>
      <div className="creation-dashboard-grid">
        <section className="creation-dashboard-card" aria-label="生产概览统计">
          <div className="creation-dashboard-card-heading"><strong>生产概览</strong><button type="button" className="quiet-button" onClick={() => void refresh()} disabled={loading}>{loading ? "刷新中..." : "刷新"}</button></div>
          <div className="creation-dashboard-stats">
            <span>运行中队列 <strong>{summary.runningCount}</strong></span>
            <span>等待任务 <strong>{overview?.pendingItems ?? 0}</strong></span>
            <span>执行中任务 <strong>{summary.activeItemCount}</strong></span>
            <span>成功任务 <strong>{summary.succeededItemCount}</strong></span>
            <span>失败任务 <strong>{summary.failedItemCount}</strong></span>
            <span>队列 <strong>{summary.queueCount}</strong></span>
          </div>
          <div className="creation-dashboard-queue-list">
            <strong>最近队列</strong>
            {recentQueues.map((queue) => {
              const action = productionQueueAction(queue.status, queue.archivedAt);
              return <div key={queue.id} className="creation-dashboard-row"><div><strong>{queue.name}</strong><small>{queue.archivedAt ? "已归档" : productionStatusLabel(queue.status)} · {formatDateTime(queue.updatedAt)}</small></div><button type="button" className="quiet-button" onClick={() => void actOnQueue(queue)} disabled={busyQueueId === queue.id}>{busyQueueId === queue.id ? "处理中..." : action}</button></div>;
            })}
            {!recentQueues.length && <p className="disabled-note">当前项目还没有生产队列。</p>}
          </div>
        </section>

        <section className="creation-dashboard-card" aria-label="最近使用工作流">
          <div className="creation-dashboard-card-heading"><strong>最近使用工作流</strong><small>成功任务历史 · 最多 5 个</small></div>
          <div className="creation-dashboard-list">
            {recentWorkflows.map((record) => {
              const recipe = catalog.find((item) => item.workflowVersionId === record.workflowVersionId && item.recipeId === record.recipeId);
              return <div key={`${record.workflowVersionId}:${record.recipeId}`} className="creation-dashboard-row"><div><strong>{workflowDisplayName(recipe?.workflowId, record.workflowName)}</strong><small>{recipe ? "当前可用" : "当前不可用"} · {formatDateTime(record.lastUsedAt)}</small></div><button type="button" className="quiet-button" onClick={() => onContinueWorkflow(record, recipe)} disabled={!recipe}>{recipe ? "继续创作" : "当前不可用"}</button></div>;
            })}
            {!recentWorkflows.length && <p className="disabled-note">暂无成功任务历史。</p>}
          </div>
        </section>

        <section className="creation-dashboard-card" aria-label="最近提示词">
          <div className="creation-dashboard-card-heading"><strong>最近提示词</strong><small>提示词 / 片段 · 最多 5 个</small></div>
          {textFields.length > 1 && <label className="creation-dashboard-target"><span>用于创作目标字段</span><select value={promptTargetFieldKey} onChange={(event) => onPromptTargetFieldChange(event.target.value)}><option value="">请选择文字字段</option>{textFields.map((field) => <option key={field.key} value={field.key}>{fieldLabel(field.key, field.label)}</option>)}</select></label>}
          <div className="creation-dashboard-list">
            {recentPrompts.map((entry) => <div key={entry.id} className="creation-dashboard-row"><div><strong>{entry.name}</strong><small>{entry.kind === "prompt" ? "提示词" : "片段"} · {entry.versionCount} 个版本 · {formatDateTime(entry.updatedAt)}</small></div><button type="button" className="quiet-button" onClick={() => onUsePrompt(entry, promptTargetFieldKey || textFields[0]?.key || "")} disabled={!textFields.length || (textFields.length > 1 && !promptTargetFieldKey)}>用于创作</button></div>)}
            {!recentPrompts.length && <p className="disabled-note">暂无提示词库条目。</p>}
          </div>
        </section>
      </div>
      {error && <p className="error-message" role="alert">生产概览：{error}</p>}
    </details>
  );
}
