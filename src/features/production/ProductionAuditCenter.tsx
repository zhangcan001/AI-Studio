import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getProductionAuditIntegrity,
  getProductionAuditLineage,
  getProductionAuditRecentActivity,
  getProductionAuditSnapshotDetail,
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
  ProductionAuditSnapshotDetail,
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
  snapshotDetails?: Record<string, ProductionAuditSnapshotDetail>;
  snapshotLoadingId?: string;
  snapshotError?: string;
  loading?: boolean;
  error?: string;
  onOpenTask?: (taskId: string) => void;
  onOpenShot?: (shotId: string) => void;
  onInspectLineage?: (rootType: ProductionAuditRootType, rootId: string) => void;
  onLoadSnapshotDetail?: (node: ProductionAuditLineageNode) => void;
  onCopyContextHash?: (contextHash: string) => void | Promise<void>;
}

type ActivityFilter = "ALL" | "FAILED" | "RUNNING" | "RETRIED";

const activityFilters: Array<{ value: ActivityFilter; label: string }> = [
  { value: "ALL", label: "全部" },
  { value: "FAILED", label: "失败" },
  { value: "RUNNING", label: "运行中" },
  { value: "RETRIED", label: "重试" },
];

const rootTypes: Array<{ value: ProductionAuditRootType; label: string }> = [
  { value: "RUN", label: "运行" },
  { value: "BATCH", label: "批次" },
  { value: "SHOT", label: "镜头" },
  { value: "TASK", label: "任务" },
];

export function ProductionAuditCenter({ projectId, onOpenTask, onOpenShot }: Props) {
  const [summary, setSummary] = useState<ProductionAuditSummary>();
  const [activity, setActivity] = useState<ProductionAuditActivity[]>([]);
  const [integrity, setIntegrity] = useState<ProductionAuditIntegrity>();
  const [lineage, setLineage] = useState<ProductionAuditLineage>();
  const [snapshotDetails, setSnapshotDetails] = useState<Record<string, ProductionAuditSnapshotDetail>>({});
  const [snapshotLoadingId, setSnapshotLoadingId] = useState<string>();
  const [snapshotError, setSnapshotError] = useState<string>();
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
    setSnapshotDetails({});
    setSnapshotLoadingId(undefined);
    setSnapshotError(undefined);
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

  async function loadSnapshotDetail(node: ProductionAuditLineageNode) {
    if (snapshotDetails[node.id] || !node.itemId) return;
    setSnapshotLoadingId(node.id);
    setSnapshotError(undefined);
    try {
      const detail = await getProductionAuditSnapshotDetail({
        projectId,
        productionBatchItemId: node.itemId,
      });
      if (detail) {
        setSnapshotDetails((current) => ({ ...current, [node.id]: detail }));
      } else {
        setSnapshotError("未找到该逻辑项的生产准备快照。");
      }
    } catch (loadError: unknown) {
      setSnapshotError(toUserMessage(loadError));
    } finally {
      setSnapshotLoadingId(undefined);
    }
  }

  async function copyContextHash(contextHash: string) {
    try {
      await navigator.clipboard?.writeText(contextHash);
    } catch (copyError: unknown) {
      setError(toUserMessage(copyError));
    }
  }

  return (
    <ProductionAuditCenterView
      summary={summary}
      activity={activity}
      integrity={integrity}
      lineage={lineage}
      snapshotDetails={snapshotDetails}
      snapshotLoadingId={snapshotLoadingId}
      snapshotError={snapshotError}
      loading={loading}
      error={error}
      onOpenTask={onOpenTask}
      onOpenShot={onOpenShot}
      onInspectLineage={(nextRootType, nextRootId) => void inspectLineage(nextRootType, nextRootId)}
      onLoadSnapshotDetail={(node) => void loadSnapshotDetail(node)}
      onCopyContextHash={(contextHash) => void copyContextHash(contextHash)}
    />
  );
}

export function ProductionAuditCenterView({
  summary,
  activity = [],
  integrity,
  lineage,
  snapshotDetails = {},
  snapshotLoadingId,
  snapshotError,
  loading = false,
  error,
  onOpenTask,
  onOpenShot,
  onInspectLineage,
  onLoadSnapshotDetail,
  onCopyContextHash,
}: ProductionAuditCenterViewProps) {
  const [filter, setFilter] = useState<ActivityFilter>("ALL");
  const [keyword, setKeyword] = useState("");
  const [rootType, setRootType] = useState<ProductionAuditRootType>("RUN");
  const [rootId, setRootId] = useState("");
  const [copiedContextHash, setCopiedContextHash] = useState<string>();
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
          <span className="section-label">生产 / 任务历史</span>
          <h2>生产审计</h2>
          <p className="section-description">跨运行、阶段、批次、逻辑项、尝试、任务、快照与素材的只读生产链视图。</p>
        </div>
        {health && <HealthBadge health={health} />}
      </div>

      {error && <p className="error-message" role="alert">生产审计加载失败：{error}</p>}

      {summary ? <SummaryCards summary={summary} /> : !loading && <p className="empty-state">当前项目暂无生产审计数据。</p>}

      <section className="production-audit-section" aria-label="最近活动">
        <div className="production-audit-section-heading">
          <div><span className="section-label">最近活动</span><h3>最近活动</h3></div>
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
          <input value={keyword} onChange={(event) => setKeyword(event.target.value)} placeholder="对象、任务、镜头或错误码" />
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

      <section className="production-audit-section" aria-label="生产链路检查">
        <div className="production-audit-section-heading">
          <div><span className="section-label">生产链路检查</span><h3>生产链路</h3></div>
          <span className="production-audit-muted">运行 → 阶段 → 批次 → 逻辑项 → 尝试 → 任务 → 快照 → 素材</span>
        </div>
        <div className="production-audit-lineage-query">
          <label><span>根对象</span><select value={rootType} onChange={(event) => setRootType(event.target.value as ProductionAuditRootType)}>{rootTypes.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}</select></label>
          <label className="production-audit-id-input"><span>对象 ID</span><input value={rootId} onChange={(event) => setRootId(event.target.value)} placeholder="输入运行 / 批次 / 镜头 / 任务 ID" /></label>
          <button type="button" onClick={() => rootId.trim() && onInspectLineage?.(rootType, rootId.trim())} disabled={!rootId.trim() || !onInspectLineage || loading}>{onInspectLineage ? "查看链路" : "只读链路"}</button>
        </div>
        {lineage ? (
          <>
            <LineageTree
              nodes={lineage.nodes}
              onOpenTask={onOpenTask}
              onOpenShot={onOpenShot}
              snapshotDetails={snapshotDetails}
              snapshotLoadingId={snapshotLoadingId}
              snapshotError={snapshotError}
              copiedContextHash={copiedContextHash}
              onLoadSnapshotDetail={onLoadSnapshotDetail}
              onCopyContextHash={async (contextHash) => {
                await onCopyContextHash?.(contextHash);
                setCopiedContextHash(contextHash);
              }}
            />
            {!lineage.nodes.some(isPreparationSnapshotNode) && <p className="production-audit-muted">旧版生产记录，无准备快照</p>}
          </>
        ) : <p className="empty-state">选择最近活动或输入对象 ID 查看生产链路。</p>}
      </section>

      <section className="production-audit-section" aria-label="完整性检查">
        <div className="production-audit-section-heading">
          <div><span className="section-label">完整性审计</span><h3>完整性检查</h3></div>
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
    ["任务", summary.tasks],
    ["素材", summary.assets],
  ] as const;
  return (
    <div className="production-audit-summary-grid" aria-label="生产审计摘要">
      {cards.map(([label, value]) => <article className="production-audit-summary-card" key={label}><span>{label}</span><strong>{value}</strong></article>)}
    </div>
  );
}

function HealthBadge({ health }: { health: ProductionAuditHealth }) {
  const label = health === "HEALTHY" ? "健康" : health === "WARNING" ? "需要关注" : "已阻断";
  return <span className={`production-audit-health production-audit-health-${health.toLowerCase()}`} aria-label={`生产数据健康：${label}`}><span aria-hidden="true" />生产数据健康：{label}</span>;
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
      <div className="production-audit-activity-main"><div><strong>{item.title}</strong><span className="production-audit-kind">{auditActivityLabel(item.kind)}</span></div><p>{item.detail}</p></div>
      <div className="production-audit-activity-object">{item.errorCode && <code>{item.errorCode}</code>}{item.taskId && <button type="button" className="quiet-button" onClick={() => onOpenTask?.(item.taskId!)} disabled={!onOpenTask}>任务</button>}{item.shotId && <button type="button" className="quiet-button" onClick={() => onOpenShot?.(item.shotId!)} disabled={!onOpenShot}>查看镜头</button>}{onInspect && <button type="button" className="quiet-button" onClick={onInspect}>查看链路</button>}</div>
    </article>
  );
}

function LineageTree({
  nodes,
  onOpenTask,
  onOpenShot,
  snapshotDetails,
  snapshotLoadingId,
  snapshotError,
  copiedContextHash,
  onLoadSnapshotDetail,
  onCopyContextHash,
}: {
  nodes: ProductionAuditLineageNode[];
  onOpenTask?: (taskId: string) => void;
  onOpenShot?: (shotId: string) => void;
  snapshotDetails: Record<string, ProductionAuditSnapshotDetail>;
  snapshotLoadingId?: string;
  snapshotError?: string;
  copiedContextHash?: string;
  onLoadSnapshotDetail?: (node: ProductionAuditLineageNode) => void;
  onCopyContextHash?: (contextHash: string) => void | Promise<void>;
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
        <span className="production-audit-lineage-type">{auditEntityLabel(node.entityType)}</span>
        <strong>{node.label}</strong>
        {node.status && <span className="production-audit-lineage-status">{auditStatusLabel(node.status)}</span>}
        {node.taskId && onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(node.taskId!)}>任务详情</button>}
        {node.shotId && <span className="production-audit-shot-link">镜头 {node.shotId}{onOpenShot && <button type="button" className="quiet-button" onClick={() => onOpenShot(node.shotId!)}>查看镜头</button>}</span>}
        {isPreparationSnapshotNode(node) && <SnapshotLineageSummary node={node} detail={snapshotDetails[node.id] ?? (node.snapshotId ? snapshotDetails[node.snapshotId] : undefined)} loading={snapshotLoadingId === node.id} error={snapshotError} copiedContextHash={copiedContextHash} onLoadDetail={onLoadSnapshotDetail} onCopyContextHash={onCopyContextHash} />}
      </div>
      )}
    </div>
  );
}

function SnapshotLineageSummary({
  node,
  detail,
  loading,
  error,
  copiedContextHash,
  onLoadDetail,
  onCopyContextHash,
}: {
  node: ProductionAuditLineageNode;
  detail?: ProductionAuditSnapshotDetail;
  loading: boolean;
  error?: string;
  copiedContextHash?: string;
  onLoadDetail?: (node: ProductionAuditLineageNode) => void;
  onCopyContextHash?: (contextHash: string) => void | Promise<void>;
}) {
  const contextHash = node.contextHash ?? detail?.contextHash;
  const hasDetail = Boolean(detail);
  return (
    <div className="production-audit-snapshot-summary">
      {contextHash && <code title={contextHash}>contextHash: {contextHash}</code>}
      {node.stage && <span className="production-audit-lineage-status">阶段 {node.stage}</span>}
      {node.batchId && <span className="production-audit-lineage-status">批次 {node.batchId}</span>}
      {node.itemId && <span className="production-audit-lineage-status">逻辑项 {node.itemId}</span>}
      {node.createdAt && <time className="production-audit-lineage-status" dateTime={node.createdAt}>创建于 {formatDateTime(node.createdAt)}</time>}
      {contextHash && <button type="button" className="quiet-button" onClick={() => void onCopyContextHash?.(contextHash)}>{copiedContextHash === contextHash ? "已复制" : "复制 contextHash"}</button>}
      <button type="button" className="quiet-button" onClick={() => onLoadDetail?.(node)} disabled={!onLoadDetail || !node.itemId || loading} aria-expanded={hasDetail}>{loading ? "正在读取快照" : hasDetail ? "保留快照详情" : "查看快照详情"}</button>
      {error && <small role="alert">快照详情读取失败：{error}</small>}
      {detail && <SnapshotDetail detail={detail} />}
    </div>
  );
}

function SnapshotDetail({ detail }: { detail: ProductionAuditSnapshotDetail }) {
  return (
    <dl className="production-audit-snapshot-detail">
      {detail.prompt && <div><dt>提示词</dt><dd>{detail.prompt}</dd></div>}
      {detail.negativePrompt && <div><dt>负面提示</dt><dd>{detail.negativePrompt}</dd></div>}
      {detail.workflowVersionId && <div><dt>工作流</dt><dd>{detail.workflowVersionId}</dd></div>}
      {detail.recipeId && <div><dt>配方</dt><dd>{detail.recipeId}</dd></div>}
      {detail.referenceSetIds.length ? <div><dt>参考集</dt><dd>{detail.referenceSetIds.join("、")}</dd></div> : null}
      {detail.assetChecksums.length ? <div><dt>素材校验</dt><dd>{detail.assetChecksums.join("、")}</dd></div> : null}
    </dl>
  );
}

function isPreparationSnapshotNode(node: ProductionAuditLineageNode): boolean {
  return node.entityType === "PREPARATION_SNAPSHOT";
}

function IssueList({ issues }: { issues: ProductionAuditIssue[] }) {
  return <div className="production-audit-issue-list">{issues.map((issue) => <article className={`production-audit-issue production-audit-severity-${issue.severity.toLowerCase()}`} key={`${issue.code}:${issue.entityType}:${issue.entityId}`}><strong>{issue.code}</strong><span>{issue.message}</span><small>{auditEntityLabel(issue.entityType)} · {issue.entityId}{issue.relatedIds.length ? ` · 关联 ${issue.relatedIds.join(", ")}` : ""}</small></article>)}</div>;
}

function auditEntityLabel(entityType: string): string {
  const labels: Record<string, string> = {
    RUN: "运行",
    STAGE: "阶段",
    BATCH: "批次",
    LOGICAL_ITEM: "逻辑项",
    ATTEMPT: "尝试",
    TASK: "任务",
    SNAPSHOT: "快照",
    PREPARATION_SNAPSHOT: "生产准备快照",
    ASSET: "素材",
    SHOT: "镜头",
  };
  return labels[entityType] ?? entityType;
}

function auditActivityLabel(kind: string): string {
  const labels: Record<string, string> = {
    RUN_CREATED: "运行已创建",
    RUN_COMPLETED: "运行已完成",
    RUN_FAILED: "运行失败",
    BATCH_CREATED: "批次已创建",
    BATCH_PAUSED: "批次已暂停",
    BATCH_COMPLETED: "批次已完成",
    ITEM_FAILED: "项目失败",
    ITEM_RETRIED: "项目已重试",
    TASK_SUCCEEDED: "任务已完成",
    TASK_FAILED: "任务失败",
    ASSET_CREATED: "素材已创建",
    PREPARATION_CREATED: "生产准备快照已创建",
    SHOT_IMAGE_SELECTED: "已选择镜头图片",
    SHOT_VIDEO_SELECTED: "已选择镜头视频",
  };
  return labels[kind] ?? kind;
}

function auditStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    READY: "就绪",
    RUNNING: "运行中",
    QUEUED: "已排队",
    WAITING: "等待中",
    SUCCEEDED: "已完成",
    COMPLETED: "已完成",
    FAILED: "失败",
    CANCELLED: "已取消",
    PAUSED: "已暂停",
  };
  return labels[status] ?? status;
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
