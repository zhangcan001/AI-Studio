import { useEffect, useState } from "react";
import type { TaskView } from "../../types/task";

const STATUS_LABELS: Record<TaskView["status"], string> = {
  CREATED: "Creating task",
  VALIDATING: "Validating parameters",
  PREPARING: "Preparing generation",
  QUEUED: "Queued",
  RUNNING: "Generating",
  COLLECTING: "Collecting output",
  SUCCEEDED: "Complete",
  FAILED: "Failed",
};

interface Props {
  task?: TaskView;
}

export function TaskProgressCard({ task }: Props) {
  const [, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!task || task.status === "SUCCEEDED" || task.status === "FAILED") return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [task]);

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
        <span className={`status-pill task-${task.status.toLowerCase()}`}>{task.status}</span>
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
      ) : task.status === "RUNNING" ? (
        <p className="indeterminate-message">Processing...</p>
      ) : null}
      <div className="task-meta">
        <span>Elapsed {formatElapsed(task, Date.now())}</span>
        {task.queueNumber !== undefined && <span>Queue #{task.queueNumber}</span>}
      </div>
    </section>
  );
}

function formatElapsed(task: TaskView, now: number): string {
  const start = Date.parse(task.startedAt ?? task.createdAt);
  const end = task.finishedAt ? Date.parse(task.finishedAt) : now;
  const seconds = Math.max(0, Math.floor((end - start) / 1000));
  return `${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, "0")}s`;
}
