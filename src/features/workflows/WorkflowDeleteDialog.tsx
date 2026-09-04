import { useEffect } from "react";
import type {
  WorkflowDeletionInspection,
  WorkflowProductionWorkspaceView,
} from "../../types/workflowOnboarding";

interface Props {
  item: WorkflowProductionWorkspaceView;
  inspection: WorkflowDeletionInspection;
  deleting?: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

function sourceLabel(item: WorkflowProductionWorkspaceView, inspection: WorkflowDeletionInspection): string {
  return item.builtin || inspection.builtin ? "系统自带" : "用户导入";
}

export function WorkflowDeleteDialog({ item, inspection, deleting = false, onClose, onConfirm }: Props) {
  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape" && !deleting) onClose();
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [deleting, onClose]);

  const activeTaskCount = inspection.activeTaskCount ?? 0;
  const activeQueueItemCount = inspection.activeQueueItemCount ?? 0;
  const projectBindingCount = inspection.projectBindingCount;
  const blockedByActiveWork = activeTaskCount > 0 || activeQueueItemCount > 0;
  const blocked = blockedByActiveWork || inspection.deleteAction === "BLOCKED";
  const hardDelete = inspection.deleteAction === "HARD_DELETE";
  const name = inspection.name || item.name || item.packageName;

  return (
    <div className="asset-delete-backdrop" role="presentation" onMouseDown={() => !deleting && onClose()}>
      <section
        className="asset-delete-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="workflow-delete-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="section-heading">
          <div>
            <span className="section-label">工作流库</span>
            <h2 id="workflow-delete-title">删除工作流</h2>
          </div>
          <button type="button" className="quiet-button" onClick={onClose} disabled={deleting}>关闭</button>
        </div>

        <div className="workflow-detail-grid">
          <span>工作流名称<strong>{name}</strong></span>
          <span>来源<strong>{sourceLabel(item, inspection)}</strong></span>
          <span>当前项目配置<strong>{projectBindingCount === undefined ? "未返回" : projectBindingCount}</strong></span>
          <span>活动任务<strong>{activeTaskCount}</strong></span>
          <span>活动队列<strong>{activeQueueItemCount}</strong></span>
        </div>

        {blockedByActiveWork && (
          <p className="asset-delete-warning">
            当前有活动任务或队列项目，暂时不能删除。请等待运行结束后再试；已有历史任务不会受到影响。
          </p>
        )}
        {!blockedByActiveWork && inspection.deleteAction === "BLOCKED" && (
          <p className="asset-delete-warning">
            当前工作流暂时不能删除：{inspection.blockingReasons.join(" ") || "系统正在保护仍被使用的工作流。"}
          </p>
        )}
        {projectBindingCount !== undefined && projectBindingCount > 0 && (
          <div>
            <p className="asset-delete-warning">
              该工作流当前被 {projectBindingCount} 个项目配置使用。删除后将解除这些项目工作流配置。
            </p>
            {!!inspection.projectBindingScopes?.length && <small className="disabled-note">受影响配置：{inspection.projectBindingScopes.join("、")}</small>}
          </div>
        )}

        <p className="asset-delete-confirm-copy">
          {hardDelete
            ? "该用户工作流没有历史引用，确认后将永久删除运行包和工作流记录。"
            : "删除后将从工作流库和生成页面移除。已有生产记录仍然保留。你可以稍后恢复该工作流。"}
        </p>
        {(inspection.historicalTaskCount > 0 || inspection.productionBatchItemCount > 0 || inspection.benchmarkReferenceCount > 0) && (
          <p className="asset-delete-confirm-copy">
            历史任务、生产批次和其他历史引用会保留，不会被删除。
          </p>
        )}

        <div className="asset-delete-actions">
          <button type="button" className="quiet-button" onClick={onClose} disabled={deleting}>取消</button>
          <button type="button" className="danger-button" onClick={onConfirm} disabled={blocked || deleting}>
            {deleting ? "正在删除……" : hardDelete ? "永久删除工作流" : "删除工作流"}
          </button>
        </div>
      </section>
    </div>
  );
}
