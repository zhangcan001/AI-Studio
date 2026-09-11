import { formatDateTime } from "../../i18n/statusLabels";
import type { WorkflowRecipeHistoryView } from "../../types/workflowHistory";

interface Props {
  history: WorkflowRecipeHistoryView;
  loading: boolean;
  onClose: () => void;
  onLoadMore: () => void;
  onOpenTask?: (taskId: string) => void;
}

export function RecipeHistoryPane({ history, loading, onClose, onLoadMore, onOpenTask }: Props) {
  return (
    <section className="workflow-onboarding-panel" aria-label="Recipe 历史">
      <div className="workflow-onboarding-heading">
        <div>
          <span className="section-label">只读历史</span>
          <h3>Recipe {history.recipeVersion}</h3>
          <p className="section-description">工作流版本 {history.workflowVersion} · {history.archived ? "已归档" : "可用"}{history.isPromoted ? " · 已推广" : ""}</p>
        </div>
        <button type="button" className="quiet-button" onClick={onClose}>关闭历史</button>
      </div>

      <div className="workflow-detail-grid">
        <span>任务总数 <strong>{history.taskCount}</strong></span>
        <span>进行中 <strong>{history.activeTaskCount}</strong></span>
        <span>成功 <strong>{history.succeededTaskCount}</strong></span>
        <span>失败 <strong>{history.failedTaskCount}</strong></span>
        <span>取消 <strong>{history.cancelledTaskCount}</strong></span>
        <span>最后执行 <strong>{history.lastFinishedAt ? formatDateTime(history.lastFinishedAt) : "尚未完成"}</strong></span>
        <span>执行项目数 <strong>{history.executedProjectCount}</strong></span>
        <span>关联项目数 <strong>{history.referencedProjectCount}</strong></span>
        <span>队列记录 <strong>{history.queueItemCount}</strong></span>
        <span>Preset <strong>{history.presetCount}</strong></span>
        <span>Shot 使用 <strong>{history.shotStageCount}</strong></span>
        <span>Experiment <strong>{history.experimentCount}</strong></span>
        <span>Benchmark Run <strong>{history.benchmarkRunCount}</strong></span>
      </div>

      <details className="workflow-catalog-detail" open>
        <summary>任务记录（按创建时间倒序）</summary>
        {!history.taskPage.items.length && <p className="empty-state">没有该 exact Recipe 的任务记录。</p>}
        {!!history.taskPage.items.length && <div className="workflow-recipe-summary">
          {history.taskPage.items.map((task) => (
            <span key={task.id}>
              {task.projectName} · {task.status} · {formatDateTime(task.createdAt)}
              {onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(task.id)}>查看任务</button>}
            </span>
          ))}
        </div>}
        {history.taskPage.nextCursor && <button type="button" className="quiet-button" onClick={onLoadMore} disabled={loading}>{loading ? "正在读取…" : "读取更多任务"}</button>}
      </details>

      <details className="workflow-catalog-detail">
        <summary>关联记录</summary>
        <div className="workflow-detail-grid">
          <span>Project Binding <strong>{history.projectBindingCount}</strong></span>
          <span>Project Template <strong>{history.projectTemplateCount}</strong></span>
          <span>Production Run Template <strong>{history.productionRunTemplateCount}</strong></span>
        </div>
        {!!history.queueItems.length && <p>最近队列记录：{history.queueItems.slice(0, 5).map((item) => `${item.batchName} · ${item.itemStatus}${item.taskId ? " · 已关联任务" : " · 计划使用"}`).join("；")}</p>}
        {!!history.projectBindings.length && <p>项目绑定：{history.projectBindings.slice(0, 5).map((binding) => `${binding.projectName} · ${binding.stage}/${binding.mode}`).join("；")}</p>}
        {!!history.shots.length && <p>Shot：{history.shots.slice(0, 5).map((shot) => `${shot.projectName} · ${shot.shotId} · ${shot.stage}`).join("；")}</p>}
        {!!history.experiments.length && <p>Experiment：{history.experiments.slice(0, 5).map((experiment) => `${experiment.experimentName} · ${experiment.candidateId}`).join("；")}</p>}
      </details>

      <details>
        <summary>技术详情</summary>
        <dl>
          <dt>workflowVersionId</dt><dd><code>{history.workflowVersionId}</code></dd>
          <dt>recipeId</dt><dd><code>{history.recipeId}</code></dd>
          <dt>recipeSha256</dt><dd><code>{history.recipeSha256}</code></dd>
        </dl>
      </details>
    </section>
  );
}
