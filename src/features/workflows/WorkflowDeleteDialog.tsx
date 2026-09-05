import { useEffect, useState } from "react";
import type {
  WorkflowDeletionInspection,
  WorkflowPurgeInspection,
  WorkflowProductionWorkspaceView,
} from "../../types/workflowOnboarding";

export type WorkflowDeletionMode = "REMOVE" | "PURGE";

interface Props {
  item: WorkflowProductionWorkspaceView;
  inspection: WorkflowDeletionInspection | WorkflowPurgeInspection;
  mode: WorkflowDeletionMode;
  deleting?: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

function sourceLabel(
  item: WorkflowProductionWorkspaceView,
  inspection: WorkflowDeletionInspection | WorkflowPurgeInspection,
  mode: WorkflowDeletionMode,
): string {
  return item.builtin || (mode === "REMOVE" && (inspection as WorkflowDeletionInspection).builtin) ? "系统自带" : "用户导入";
}

export function WorkflowDeleteDialog({ item, inspection, mode, deleting = false, onClose, onConfirm }: Props) {
  const [typedConfirmation, setTypedConfirmation] = useState("");

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape" && !deleting) onClose();
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [deleting, onClose]);
  useEffect(() => {
    setTypedConfirmation("");
  }, [inspection.workflowId, mode]);

  const purge = mode === "PURGE";
  const purgeInspection = inspection as WorkflowPurgeInspection;
  const deletionInspection = inspection as WorkflowDeletionInspection;
  const activeTaskCount = purge ? 0 : deletionInspection.activeTaskCount ?? 0;
  const activeQueueItemCount = purge ? 0 : deletionInspection.activeQueueItemCount ?? 0;
  const projectBindingCount = purge ? purgeInspection.bindingCount : deletionInspection.projectBindingCount;
  const blockedByActiveWork = !purge && (activeTaskCount > 0 || activeQueueItemCount > 0);
  const blocked = purge
    ? !purgeInspection.canPurge
    : blockedByActiveWork || deletionInspection.deleteAction === "BLOCKED";
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
            <h2 id="workflow-delete-title">{purge ? "彻底删除工作流" : "删除工作流"}</h2>
          </div>
          <button type="button" className="quiet-button" onClick={onClose} disabled={deleting}>关闭</button>
        </div>

        <div className="workflow-detail-grid">
          <span>工作流名称<strong>{name}</strong></span>
          <span>来源<strong>{sourceLabel(item, inspection, mode)}</strong></span>
          <span>{purge ? "运行包" : "当前项目配置"}<strong>{purge ? purgeInspection.packageCount : projectBindingCount === undefined ? "未返回" : projectBindingCount}</strong></span>
          {!purge && <span>活动任务<strong>{activeTaskCount}</strong></span>}
          {!purge && <span>活动队列<strong>{activeQueueItemCount}</strong></span>}
        </div>

        {blockedByActiveWork && (
          <p className="asset-delete-warning">
            当前有活动任务或队列项目，暂时不能删除。请等待运行结束后再试；已有历史任务不会受到影响。
          </p>
        )}
        {!purge && !blockedByActiveWork && deletionInspection.deleteAction === "BLOCKED" && (
          <p className="asset-delete-warning">
            当前工作流暂时不能删除：{deletionInspection.blockingReasons.join(" ") || "系统正在保护仍被使用的工作流。"}
          </p>
        )}
        {purge && !purgeInspection.canPurge && (
          <p className="asset-delete-warning">
            当前工作流不能彻底删除：{purgeInspection.blockingReasons.join(" ") || "系统正在保护仍被使用的工作流。"}
          </p>
        )}
        {projectBindingCount !== undefined && projectBindingCount > 0 && (
          <div>
            <p className="asset-delete-warning">
              该工作流当前被 {projectBindingCount} 个项目配置使用。{purge ? "彻底删除" : "删除"}后将解除这些项目工作流配置。
            </p>
            {!purge && !!deletionInspection.projectBindingScopes?.length && <small className="disabled-note">受影响配置：{deletionInspection.projectBindingScopes.join("、")}</small>}
          </div>
        )}

        <p className="asset-delete-confirm-copy">
          {purge
            ? "将永久删除该用户工作流的运行包、Recipe、工作流版本和工作流记录。完成后无法恢复。"
            : "该工作流将从工作流库和生成页面移除。历史记录和运行数据会保留，你可以稍后恢复。"}
        </p>
        {purge && <p className="asset-delete-warning">这是不可恢复操作。</p>}
        {!purge && (deletionInspection.historicalTaskCount > 0 || deletionInspection.productionBatchItemCount > 0 || deletionInspection.benchmarkReferenceCount > 0) && (
          <p className="asset-delete-confirm-copy">
            历史任务、生产批次和其他历史引用会保留，不会被删除。
          </p>
        )}

        {purge && (
          <label className="asset-delete-confirm-copy">
            输入“永久删除”以确认
            <input
              aria-label="输入“永久删除”以确认"
              value={typedConfirmation}
              onChange={(event) => setTypedConfirmation(event.target.value)}
              disabled={deleting}
            />
          </label>
        )}

        <div className="asset-delete-actions">
          <button type="button" className="quiet-button" onClick={onClose} disabled={deleting}>取消</button>
          <button type="button" className="danger-button" onClick={onConfirm} disabled={blocked || deleting || (purge && typedConfirmation !== "永久删除")}>
            {deleting ? "正在删除……" : purge ? "永久删除" : "删除工作流"}
          </button>
        </div>
      </section>
    </div>
  );
}
