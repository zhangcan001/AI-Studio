import type { TaskView } from "../../types/task";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDurationMs, taskStatusLabel } from "../../i18n/statusLabels";

interface Props {
  task?: TaskView;
  cancelling?: boolean;
  onCancel?: () => void | Promise<void>;
}

export function TaskProgressCard({ task, cancelling = false, onCancel }: Props) {
  if (!task) {
    return (
      <section className="task-card empty-task">
        <span className="section-label">任务状态</span>
        <p>开始生成后，任务进度会显示在这里。</p>
      </section>
    );
  }

  const progress = task.progress.mode === "step" && task.progress.total
    ? Math.round(((task.progress.current ?? 0) / task.progress.total) * 100)
    : undefined;

  return (
    <section className="task-card" aria-live="polite">
      <div className="task-heading">
        <div>
          <span className="section-label">任务状态</span>
          <h2>{taskStatusLabel(task.status)}</h2>
        </div>
        <div className="task-heading-actions">
          <span className={`status-pill task-${task.status.toLowerCase()}`}>{taskStatusLabel(task.status)}</span>
          {onCancel && canCancel(task.status) && (
            <button
              type="button"
              className="cancel-button"
              onClick={() => void onCancel()}
              disabled={cancelling || task.status === "CANCEL_REQUESTED"}
            >
              {cancelling ? "正在取消..." : "取消任务"}
            </button>
          )}
        </div>
      </div>
      {task.status === "FAILED" && task.error && (
        <div className="task-error">
          <strong>{task.error.code}</strong>
          <span>{toUserMessage(task.error)}</span>
        </div>
      )}
      {task.progress.mode === "step" && progress !== undefined ? (
        <div className="step-progress">
          <div className="progress-label">
          <span>采样步数</span>
            <span>{task.progress.current} / {task.progress.total}</span>
          </div>
          <div className="progress-track"><div style={{ width: `${progress}%` }} /></div>
          <small>{progress}%</small>
        </div>
      ) : task.status === "RUNNING" || task.status === "CANCEL_REQUESTED" ? (
        <p className="indeterminate-message">正在处理...</p>
      ) : null}
      <div className="task-meta">
        <span>已耗时 {formatElapsed(task, Date.now())}</span>
        {task.queueNumber !== undefined && <span>队列序号 #{task.queueNumber}</span>}
      </div>
    </section>
  );
}

function canCancel(status: TaskView["status"]): boolean {
  return ["CREATED", "VALIDATING", "PREPARING", "QUEUED", "RUNNING"].includes(status);
}

function formatElapsed(task: TaskView, now: number): string {
  const start = Date.parse(task.startedAt ?? task.createdAt);
  const end = task.finishedAt ? Date.parse(task.finishedAt) : now;
  return formatDurationMs(Math.max(0, end - start));
}
