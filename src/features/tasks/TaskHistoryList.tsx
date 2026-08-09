import type { PageCursor } from "../../types/asset";
import type { TaskHistoryFilter, TaskHistoryItem, TaskHistoryTimeFilter, TaskHistoryWorkflowOption } from "../../types/history";
import { workflowDisplayName, formatDateTime, taskStatusLabel } from "../../i18n/statusLabels";

interface Props {
  filter: TaskHistoryFilter;
  keyword: string;
  workflowId: string;
  timeFilter: TaskHistoryTimeFilter;
  workflowOptions: TaskHistoryWorkflowOption[];
  items: TaskHistoryItem[];
  nextCursor?: PageCursor;
  loading: boolean;
  onFilterChange: (filter: TaskHistoryFilter) => void;
  onKeywordChange: (keyword: string) => void;
  onWorkflowChange: (workflowId: string) => void;
  onTimeFilterChange: (filter: TaskHistoryTimeFilter) => void;
  onSelect: (taskId: string) => void;
  onRefresh: () => void;
  onLoadMore: () => void;
}

const filters: Array<{ value: TaskHistoryFilter; label: string }> = [
  { value: "ALL", label: "全部" },
  { value: "ACTIVE", label: "进行中" },
  { value: "SUCCEEDED", label: "已完成" },
  { value: "FAILED", label: "失败" },
  { value: "CANCELLED", label: "已取消" },
];

export function TaskHistoryList({
  filter,
  keyword,
  workflowId,
  timeFilter,
  workflowOptions,
  items,
  nextCursor,
  loading,
  onFilterChange,
  onKeywordChange,
  onWorkflowChange,
  onTimeFilterChange,
  onSelect,
  onRefresh,
  onLoadMore,
}: Props) {
  return (
    <>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">任务</span>
          <h2>任务历史</h2>
          <p className="section-description">查看当前项目的任务记录和已保存的生成输入。</p>
        </div>
        <button type="button" className="quiet-button" onClick={onRefresh} disabled={loading}>
          {loading ? "正在刷新..." : "刷新"}
        </button>
      </div>
      <div className="filter-row" aria-label="任务状态筛选">
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
      <div className="task-history-query" aria-label="任务查询">
        <label className="task-history-search">
          <span>搜索任务</span>
          <input value={keyword} onChange={(event) => onKeywordChange(event.target.value)} placeholder="任务 ID 或工作流名称" />
        </label>
        <label>
          <span>工作流</span>
          <select value={workflowId} onChange={(event) => onWorkflowChange(event.target.value)}>
            <option value="">全部工作流</option>
            {workflowOptions.map((option) => <option key={option.workflowId} value={option.workflowId}>{workflowDisplayName(option.workflowId, option.workflowName)}</option>)}
          </select>
        </label>
        <label>
          <span>时间</span>
          <select value={timeFilter} onChange={(event) => onTimeFilterChange(event.target.value as TaskHistoryTimeFilter)}>
            <option value="ALL">全部时间</option>
            <option value="TODAY">今天</option>
            <option value="LAST_7_DAYS">最近7天</option>
            <option value="LAST_30_DAYS">最近30天</option>
          </select>
        </label>
      </div>
      <div className="task-history-list">
        {items.map((task) => (
          <button type="button" className="task-history-row" key={task.id} onClick={() => onSelect(task.id)}>
            <span className={`status-pill task-${task.status.toLowerCase()}`}>{taskStatusLabel(task.status)}</span>
            <span className="task-history-row-main">
              <strong>{workflowDisplayName(task.workflowId, task.workflowName)}</strong>
              <small>
                {formatDateTime(task.createdAt)} · {formatDuration(task)} · {task.outputCount} 个输出结果
              </small>
            </span>
            <span className="task-history-row-id">{task.id}</span>
          </button>
        ))}
        {!loading && !items.length && <p className="empty-state">当前筛选条件下没有找到任务。</p>}
      </div>
      {nextCursor && (
        <button type="button" className="load-more-button" onClick={onLoadMore} disabled={loading}>
          {loading ? "正在加载..." : "加载更多"}
        </button>
      )}
    </>
  );
}

function formatDuration(task: TaskHistoryItem): string {
  if (!task.startedAt) return "未开始";
  const start = new Date(task.startedAt).getTime();
  const end = task.finishedAt ? new Date(task.finishedAt).getTime() : Date.now();
  const seconds = Math.max(0, Math.round((end - start) / 1000));
  if (seconds < 1) return "不到 1 秒";
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return `${minutes} 分${remainder ? ` ${remainder} 秒` : ""}`;
}
