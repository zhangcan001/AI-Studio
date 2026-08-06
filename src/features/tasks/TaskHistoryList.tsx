import type { PageCursor } from "../../types/asset";
import type { TaskHistoryFilter, TaskHistoryItem } from "../../types/history";

interface Props {
  filter: TaskHistoryFilter;
  items: TaskHistoryItem[];
  nextCursor?: PageCursor;
  loading: boolean;
  onFilterChange: (filter: TaskHistoryFilter) => void;
  onSelect: (taskId: string) => void;
  onRefresh: () => void;
  onLoadMore: () => void;
}

const filters: Array<{ value: TaskHistoryFilter; label: string }> = [
  { value: "ALL", label: "All" },
  { value: "ACTIVE", label: "Active" },
  { value: "SUCCEEDED", label: "Succeeded" },
  { value: "FAILED", label: "Failed" },
  { value: "CANCELLED", label: "Cancelled" },
];

export function TaskHistoryList({
  filter,
  items,
  nextCursor,
  loading,
  onFilterChange,
  onSelect,
  onRefresh,
  onLoadMore,
}: Props) {
  return (
    <>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">Tasks</span>
          <h2>Task History</h2>
          <p className="section-description">Project-scoped records with their saved generation inputs.</p>
        </div>
        <button type="button" className="quiet-button" onClick={onRefresh} disabled={loading}>
          {loading ? "Refreshing..." : "Refresh"}
        </button>
      </div>
      <div className="filter-row" aria-label="Task status filters">
        {filters.map((item) => (
          <button
            key={item.value}
            type="button"
            className={filter === item.value ? "filter-button filter-button-active" : "filter-button"}
            onClick={() => onFilterChange(item.value)}
          >
            {item.label}
          </button>
        ))}
      </div>
      <div className="task-history-list">
        {items.map((task) => (
          <button type="button" className="task-history-row" key={task.id} onClick={() => onSelect(task.id)}>
            <span className={`status-pill task-${task.status.toLowerCase()}`}>{task.status}</span>
            <span className="task-history-row-main">
              <strong>{task.workflowName}</strong>
              <small>
                {formatDate(task.createdAt)} · {formatDuration(task)} · {task.outputCount} output{task.outputCount === 1 ? "" : "s"}
              </small>
            </span>
            <span className="task-history-row-id">{task.id}</span>
          </button>
        ))}
        {!loading && !items.length && <p className="empty-state">No tasks found for this filter.</p>}
      </div>
      {nextCursor && (
        <button type="button" className="load-more-button" onClick={onLoadMore} disabled={loading}>
          {loading ? "Loading..." : "Load more"}
        </button>
      )}
    </>
  );
}

function formatDate(value: string): string {
  return new Date(value).toLocaleString();
}

function formatDuration(task: TaskHistoryItem): string {
  if (!task.startedAt) return "—";
  const start = new Date(task.startedAt).getTime();
  const end = task.finishedAt ? new Date(task.finishedAt).getTime() : Date.now();
  const seconds = Math.max(0, Math.round((end - start) / 1000));
  if (seconds < 1) return "<1s";
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return `${minutes}m${remainder ? ` ${remainder}s` : ""}`;
}
