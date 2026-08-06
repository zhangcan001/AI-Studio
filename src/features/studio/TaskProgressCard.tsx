import type { TaskView } from "../../types/task";

const STATUS_LABELS: Record<TaskView["status"], string> = {
  CREATED: "Creating task",
  VALIDATING: "Validating parameters",
  PREPARING: "Preparing generation",
  QUEUED: "Queued",
  RUNNING: "Generating",
  CANCEL_REQUESTED: "Cancelling",
  COLLECTING: "Collecting output",
  SUCCEEDED: "Complete",
  FAILED: "Failed",
  CANCELLED: "Cancelled",
};

interface Props {
  task?: TaskView;
  cancelling?: boolean;
  onCancel?: () => void | Promise<void>;
}

export function TaskProgressCard({ task, cancelling = false, onCancel }: Props) {
  if (!task) {
    return (
      <section className="task-card empty-task">
        <span className="section-label">Task status</span>
        <p>Generate an image to see live progress here.</p>
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
          <span className="section-label">Task status</span>
          <h2>{STATUS_LABELS[task.status]}</h2>
        </div>
        <div className="task-heading-actions">
          <span className={`status-pill task-${task.status.toLowerCase()}`}>{task.status}</span>
          {onCancel && canCancel(task.status) && (
            <button
              type="button"
              className="cancel-button"
              onClick={() => void onCancel()}
              disabled={cancelling || task.status === "CANCEL_REQUESTED"}
            >
              {cancelling ? "Cancelling..." : "Cancel"}
            </button>
          )}
        </div>
      </div>
      {task.status === "FAILED" && task.error && (
        <div className="task-error">
          <strong>{task.error.code}</strong>
          <span>{task.error.message}</span>
        </div>
      )}
      {task.progress.mode === "step" && progress !== undefined ? (
        <div className="step-progress">
          <div className="progress-label">
            <span>Sampler step</span>
            <span>{task.progress.current} / {task.progress.total}</span>
          </div>
          <div className="progress-track"><div style={{ width: `${progress}%` }} /></div>
          <small>{progress}%</small>
        </div>
      ) : task.status === "RUNNING" || task.status === "CANCEL_REQUESTED" ? (
        <p className="indeterminate-message">Processing...</p>
      ) : null}
      <div className="task-meta">
        <span>Elapsed {formatElapsed(task, Date.now())}</span>
        {task.queueNumber !== undefined && <span>Queue #{task.queueNumber}</span>}
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
  const seconds = Math.max(0, Math.floor((end - start) / 1000));
  return `${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, "0")}s`;
}
