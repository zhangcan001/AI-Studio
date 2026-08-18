import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getProductionAuditIntegrity,
  getProductionAuditLineage,
  getProductionAuditRecentActivity,
  getProductionAuditSummary,
} from "../../services/tauriClient";
import type {
  ProductionAuditActivity,
  ProductionAuditHealth,
  ProductionAuditIntegrity,
  ProductionAuditIssue,
  ProductionAuditLineage,
  ProductionAuditLineageNode,
  ProductionAuditRootType,
  ProductionAuditSummary,
} from "../../types/productionAudit";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime } from "../../i18n/statusLabels";
import "./ProductionAuditCenter.css";

interface Props {
  projectId: string;
  onOpenTask?: (taskId: string) => void;
  onOpenShot?: (shotId: string) => void;
}

export interface ProductionAuditCenterViewProps {
  summary?: ProductionAuditSummary;
  activity?: ProductionAuditActivity[];
  integrity?: ProductionAuditIntegrity;
  lineage?: ProductionAuditLineage;
  loading?: boolean;
  error?: string;
  onOpenTask?: (taskId: string) => void;
  onOpenShot?: (shotId: string) => void;
  onInspectLineage?: (rootType: ProductionAuditRootType, rootId: string) => void;
}

type ActivityFilter = "ALL" | "FAILED" | "RUNNING" | "RETRIED";

const activityFilters: Array<{ value: ActivityFilter; label: string }> = [
  { value: "ALL", label: "全部" },
  { value: "FAILED", label: "失败" },
  { value: "RUNNING", label: "运行中" },
  { value: "RETRIED", label: "重试" },
];

const rootTypes: Array<{ value: ProductionAuditRootType; label: string }> = [
  { value: "RUN", label: "Run" },
  { value: "BATCH", label: "Batch" },
  { value: "SHOT", label: "Shot" },
  { value: "TASK", label: "Task" },
];

export function ProductionAuditCenter({ projectId, onOpenTask, onOpenShot }: Props) {
  const [summary, setSummary] = useState<ProductionAuditSummary>();
  const [activity, setActivity] = useState<ProductionAuditActivity[]>([]);
  const [integrity, setIntegrity] = useState<ProductionAuditIntegrity>();
  const [lineage, setLineage] = useState<ProductionAuditLineage>();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(undefined);
    try {
      const [nextSummary, nextActivity, nextIntegrity] = await Promise.all([
        getProductionAuditSummary(projectId),
        getProductionAuditRecentActivity(projectId, 50),
        getProductionAuditIntegrity(projectId),
      ]);
      setSummary(nextSummary);
      setActivity(nextActivity);
      setIntegrity(nextIntegrity);
    } catch (loadError: unknown) {
      setError(toUserMessage(loadError));
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => {
    setLineage(undefined);
    void refresh();
  }, [projectId, refresh]);

  async function inspectLineage(nextRootType: ProductionAuditRootType, nextRootId: string) {
    const normalizedId = nextRootId.trim();
    if (!normalizedId) return;
    setError(undefined);
    try {
      setLineage(await getProductionAuditLineage({ projectId, rootType: nextRootType, rootId: normalizedId }));
    } catch (loadError: unknown) {
      setError(toUserMessage(loadError));
    }
  }

  return (
    <ProductionAuditCenterView
      summary={summary}
      activity={activity}
      integrity={integrity}
      lineage={lineage}
      loading={loading}
      error={error}
      onOpenTask={onOpenTask}
      onOpenShot={onOpenShot}
      onInspectLineage={(nextRootType, nextRootId) => void inspectLineage(nextRootType, nextRootId)}
    />
  );
}

export function ProductionAuditCenterView({
  summary,
  activity = [],
  integrity,
  lineage,
  loading = false,
  error,
  onOpenTask,
  onOpenShot,
  onInspectLineage,
}: ProductionAuditCenterViewProps) {
  const [filter, setFilter] = useState<ActivityFilter>("ALL");
  const [keyword, setKeyword] = useState("");
  const [rootType, setRootType] = useState<ProductionAuditRootType>("RUN");
  const [rootId, setRootId] = useState("");
  const visibleActivity = useMemo(
    () => filterActivities(activity, filter, keyword),
    [activity, filter, keyword],
  );
  const issues = integrity?.issues.length ? integrity.issues : summary?.issues ?? [];
  const health = summary?.health ?? integrity?.health;

  function inspectFromActivity(item: ProductionAuditActivity) {
    const candidate = activityRoot(item);
    if (!candidate) return;
    setRootType(candidate.rootType);
    setRootId(candidate.rootId);
    onInspectLineage?.(candidate.rootType, candidate.rootId);
  }

  return (
    <section className="workspace-panel production-audit-center" aria-busy={loading || undefined}>
      <div className="section-heading workspace-heading production-audit-heading">
        <div>
          <span className="section-label">Production / Task History</span>
          <h2>生产审计</h2>
          <p className="section-description">跨 Run、Batch、Shot、Task、Snapshot 与 Asset 的只读生产链视图。</p>
        </div>
        {health && <HealthBadge health={health} />}
      </div>

      {error && <p className="error-message" role="alert">生产审计加载失败：{error}</p>}

      {summary ? <SummaryCards summary={summary} /> : !loading && <p className="empty-state">当前项目暂无生产审计数据。</p>}

      <section className="production-audit-section" aria-label="最近活动">
        <div className="production-audit-section-heading">
          <div><span className="section-label">Recent Activity</span><h3>最近活动</h3></div>
          <span className="production-audit-muted">最多显示 50 条可靠时间记录</span>
        </div>
        <div className="filter-row" aria-label="审计活动筛选">
          {activityFilters.map((item) => (
            <button
              type="button"
              key={item.value}
              className={filter === item.value ? "filter-button filter-button-active" : "filter-button"}
              onClick={() => setFilter(item.value)}
            >
              {item.label}
            </button>
          ))}
        </div>
        <label className="production-audit-search">
          <span>搜索活动</span>
          <input value={keyword} onChange={(event) => setKeyword(event.target.value)} placeholder="对象、Task、Shot 或错误码" />
        </label>
        {visibleActivity.length ? (
          <div className="production-audit-activity-list">
            {visibleActivity.map((item) => (
              <ActivityRow
                key={item.id}
                item={item}
                onOpenTask={onOpenTask}
                onOpenShot={onOpenShot}
                onInspect={onInspectLineage ? () => inspectFromActivity(item) : undefined}
              />
            ))}
          </div>
        ) : (
          <p className="empty-state">当前筛选条件下没有审计活动。</p>
        )}
      </section>

      <section className="production-audit-section" aria-label="Lineage Inspector">
        <div className="production-audit-section-heading">
          <div><span className="section-label">Lineage Inspector</span><h3>生产链路</h3></div>
          <span className="production-audit-muted">Run → Stage → Batch → Logical Item → Attempt → Task → Snapshot → Asset</span>
        </div>
        <div className="production-audit-lineage-query">
          <label><span>根对象</span><select value={rootType} onChange={(event) => setRootType(event.target.value as ProductionAuditRootType)}>{rootTypes.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}</select></label>
          <label className="production-audit-id-input"><span>对象 ID</span><input value={rootId} onChange={(event) => setRootId(event.target.value)} placeholder="输入 Run / Batch / Shot / Task ID" /></label>
          <button type="button" onClick={() => rootId.trim() && onInspectLineage?.(rootType, rootId.trim())} disabled={!rootId.trim() || !onInspectLineage || loading}>{onInspectLineage ? "查看链路" : "只读链路"}</button>
        </div>
        {lineage ? <LineageTree nodes={lineage.nodes} onOpenTask={onOpenTask} onOpenShot={onOpenShot} /> : <p className="empty-state">选择最近活动或输入对象 ID 查看生产链路。</p>}
      </section>

      <section className="production-audit-section" aria-label="完整性检查">
        <div className="production-audit-section-heading">
          <div><span className="section-label">Integrity Audit</span><h3>完整性检查</h3></div>
          {integrity && <HealthBadge health={integrity.health} />}
        </div>
        {issues.length ? <IssueList issues={issues} /> : <p className="production-audit-healthy">未发现明确结构性断链。</p>}
      </section>
    </section>
  );
}

function SummaryCards({ summary }: { summary: ProductionAuditSummary }) {
  const cards = [
    ["运行中", summary.activeRuns + summary.activeBatches],
    ["成功", summary.completedRuns + summary.succeededItems + summary.succeededTasks],
    ["失败", summary.failedRuns + summary.failedBatches + summary.failedItems + summary.failedTasks],
    ["暂停", summary.pausedBatches],
    ["待人工检查", summary.reviewRequiredItems],
    ["Tasks", summary.tasks],
    ["Assets", summary.assets],
  ] as const;
  return (
    <div className="production-audit-summary-grid" aria-label="生产审计摘要">
      {cards.map(([label, value]) => <article className="production-audit-summary-card" key={label}><span>{label}</span><strong>{value}</strong></article>)}
    </div>
  );
}

function HealthBadge({ health }: { health: ProductionAuditHealth }) {
  const label = health === "HEALTHY" ? "健康" : health === "WARNING" ? "需要关注" : "已阻断";
  return <span className={`production-audit-health production-audit-health-${health.toLowerCase()}`} aria-label={`生产数据健康：${health}`}><span aria-hidden="true" />生产数据健康：{health} · {label}</span>;
}

function ActivityRow({
  item,
  onOpenTask,
  onOpenShot,
  onInspect,
}: {
  item: ProductionAuditActivity;
  onOpenTask?: (taskId: string) => void;
  onOpenShot?: (shotId: string) => void;
  onInspect?: () => void;
}) {
  return (
    <article className={`production-audit-activity production-audit-severity-${item.severity.toLowerCase()}`}>
      <div className="production-audit-activity-time">{formatDateTime(item.timestamp)}</div>
      <div className="production-audit-activity-main"><div><strong>{item.title}</strong><span className="production-audit-kind">{item.kind}</span></div><p>{item.detail}</p></div>
      <div className="production-audit-activity-object">{item.errorCode && <code>{item.errorCode}</code>}{item.taskId && <button type="button" className="quiet-button" onClick={() => onOpenTask?.(item.taskId!)} disabled={!onOpenTask}>任务</button>}{item.shotId && <button type="button" className="quiet-button" onClick={() => onOpenShot?.(item.shotId!)} disabled={!onOpenShot}>查看镜头</button>}{onInspect && <button type="button" className="quiet-button" onClick={onInspect}>查看链路</button>}</div>
    </article>
  );
}

function LineageTree({
  nodes,
  onOpenTask,
  onOpenShot,
}: {
  nodes: ProductionAuditLineageNode[];
  onOpenTask?: (taskId: string) => void;
  onOpenShot?: (shotId: string) => void;
}) {
  const parentById = new Map(nodes.map((node) => [node.id, node.parentId]));
  const depthOf = (node: ProductionAuditLineageNode) => {
    let depth = 0;
    let parentId = node.parentId;
    const seen = new Set<string>();
    while (parentId && !seen.has(parentId)) {
      seen.add(parentId);
      depth += 1;
      parentId = parentById.get(parentId);
    }
    return depth;
  };
  return (
    <div className="production-audit-lineage-tree" role="tree">
      {nodes.map((node) => <div className="production-audit-lineage-node" key={`${node.entityType}:${node.id}`} style={{ marginLeft: depthOf(node) * 18 }}>
        <span className="production-audit-lineage-type">{node.entityType}</span>
        <strong>{node.label}</strong>
        {node.status && <span className="production-audit-lineage-status">{node.status}</span>}
        {node.taskId && onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(node.taskId!)}>任务详情</button>}
        {node.shotId && <span className="production-audit-shot-link">Shot {node.shotId}{onOpenShot && <button type="button" className="quiet-button" onClick={() => onOpenShot(node.shotId!)}>查看镜头</button>}</span>}
      </div>
      )}
    </div>
  );
}

function IssueList({ issues }: { issues: ProductionAuditIssue[] }) {
  return <div className="production-audit-issue-list">{issues.map((issue) => <article className={`production-audit-issue production-audit-severity-${issue.severity.toLowerCase()}`} key={`${issue.code}:${issue.entityType}:${issue.entityId}`}><strong>{issue.code}</strong><span>{issue.message}</span><small>{issue.entityType} · {issue.entityId}{issue.relatedIds.length ? ` · 关联 ${issue.relatedIds.join(", ")}` : ""}</small></article>)}</div>;
}

function filterActivities(items: ProductionAuditActivity[], filter: ActivityFilter, keyword: string): ProductionAuditActivity[] {
  const normalized = keyword.trim().toLowerCase();
  return items.filter((item) => {
    const text = `${item.kind} ${item.title} ${item.detail} ${item.taskId ?? ""} ${item.shotId ?? ""} ${item.shotName ?? ""} ${item.errorCode ?? ""}`.toLowerCase();
    if (normalized && !text.includes(normalized)) return false;
    if (filter === "FAILED") return item.severity === "ERROR" || item.kind.includes("FAILED") || item.status === "FAILED";
    if (filter === "RUNNING") return item.status === "RUNNING" || item.kind === "RUN_CREATED";
    if (filter === "RETRIED") return item.kind === "ITEM_RETRIED" || Boolean(item.retryOfItemId) || /重试|retry/i.test(text);
    return true;
  });
}

function activityRoot(item: ProductionAuditActivity): { rootType: ProductionAuditRootType; rootId: string } | undefined {
  if (item.taskId) return { rootType: "TASK", rootId: item.taskId };
  if (item.shotId) return { rootType: "SHOT", rootId: item.shotId };
  if (item.batchId) return { rootType: "BATCH", rootId: item.batchId };
  if (item.runId) return { rootType: "RUN", rootId: item.runId };
  return undefined;
}

export { filterActivities };
