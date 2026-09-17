import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getArtifactReviewQueue, submitArtifactReview } from "../../services/tauriClient";
import type { ArtifactReviewDecision, ArtifactReviewQueueItemDto, ArtifactReviewQueuePageDto } from "../../types/artifact";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime } from "../../i18n/statusLabels";
import { ArtifactActions, ArtifactMetadata, ArtifactPreview, ReviewBadge } from "./ArtifactComponents";
import type { ProjectCommandCenterNavigationRequest } from "../projects/ProjectCommandCenter";
import "./ProductionReviewInbox.css";

const SUMMARY_LIMIT = 6;
const WORKSPACE_PAGE_SIZE = 50;

interface Props {
  projectId?: string;
  onNavigate?: (request: ProjectCommandCenterNavigationRequest) => void;
  mode?: "summary" | "workspace";
}

export function ProductionReviewInbox({ projectId, onNavigate, mode = "summary" }: Props) {
  const pageSize = mode === "workspace" ? WORKSPACE_PAGE_SIZE : SUMMARY_LIMIT;
  const [page, setPage] = useState<ArtifactReviewQueuePageDto>();
  const [selectedArtifactId, setSelectedArtifactId] = useState<string>();
  const [comment, setComment] = useState("");
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string>();
  const [historyRefreshKey, setHistoryRefreshKey] = useState(0);
  const pageRef = useRef<ArtifactReviewQueuePageDto | undefined>(undefined);
  const selectedArtifactIdRef = useRef<string | undefined>(undefined);

  const load = useCallback(async (offset = 0, append = false, advanceFromId?: string) => {
    if (!projectId) return;
    setLoading(true);
    setError(undefined);
    try {
      const next = await getArtifactReviewQueue(projectId, pageSize, offset);
      const current = pageRef.current;
      const items = append && current ? [...current.items, ...next.items] : next.items;
      const complete = { ...next, items };
      const oldItems = current?.items ?? [];
      const anchorId = advanceFromId ?? selectedArtifactIdRef.current;
      const oldSelectedIndex = oldItems.findIndex((item) => item.artifact.id === anchorId);
      const nextId = advanceFromId
        ? complete.items[Math.min(Math.max(0, oldSelectedIndex), complete.items.length - 1)]?.artifact.id
        : complete.items.some((item) => item.artifact.id === selectedArtifactIdRef.current)
          ? selectedArtifactIdRef.current
          : complete.items[0]?.artifact.id;
      pageRef.current = complete;
      selectedArtifactIdRef.current = nextId;
      setPage(complete);
      setSelectedArtifactId(nextId);
      setComment(complete.items.find((item) => item.artifact.id === nextId)?.review.comment ?? "");
    } catch (cause: unknown) {
      setError(toUserMessage(cause));
    } finally {
      setLoading(false);
    }
  }, [pageSize, projectId]);

  useEffect(() => {
    setPage(undefined);
    pageRef.current = undefined;
    selectedArtifactIdRef.current = undefined;
    setSelectedArtifactId(undefined);
    setComment("");
    void load();
  }, [load]);

  const items = page?.items ?? [];
  const selected = useMemo(
    () => items.find((item) => item.artifact.id === selectedArtifactId),
    [items, selectedArtifactId],
  );

  async function submit(decision: Exclude<ArtifactReviewDecision, "PENDING">) {
    if (!projectId || !selected || submitting || selected.artifact.availability !== "available") return;
    if (decision === "REJECTED" && !comment.trim()) {
      setError("驳回时请填写原因，便于后续处理。");
      return;
    }
    setSubmitting(true);
    setError(undefined);
    try {
      await submitArtifactReview({
        projectId,
        artifactId: selected.artifact.id,
        decision,
        comment: comment.trim(),
        expectedRevision: selected.review.revision,
      });
      setHistoryRefreshKey((value) => value + 1);
      await load(0, false, selected.artifact.id);
    } catch (cause: unknown) {
      setError(toUserMessage(cause));
    } finally {
      setSubmitting(false);
    }
  }

  if (!projectId) return null;
  const navigate = (request: ProjectCommandCenterNavigationRequest) => onNavigate?.({ ...request, projectId });

  if (mode === "summary") {
    return <section className="project-command-card production-review-inbox" aria-labelledby="production-review-inbox-title">
      <div className="project-command-card-heading"><div><span className="section-label">产物审核</span><h3 id="production-review-inbox-title">待审核产物</h3><p>{page ? `${page.total} 个产物等待审核` : "正在读取产物审核队列…"}</p></div>
        <div className="production-review-inbox-heading-actions">
          {page && page.total > 0 && <button type="button" className="quiet-button" onClick={() => navigate({ destination: "shots", section: "review", collectionFilter: { kind: "review", state: "PENDING" } })} disabled={!onNavigate}>查看审核队列</button>}
          <button type="button" className="quiet-button" onClick={() => void load()} disabled={loading}>刷新</button>
        </div>
      </div>
      {error && <p role="alert" className="artifact-review-error">{error}</p>}
      {items.length === 0 && !loading && !error && <p className="disabled-note">当前项目没有待审核产物。每个已登记的生成输出都会单独进入审核队列。</p>}
      <div className="production-review-inbox-list">
        {items.map((item) => <SummaryItem key={item.artifact.id} projectId={projectId} item={item} />)}
      </div>
          {page && items.length < page.total && <button type="button" className="quiet-button" onClick={() => void load(items.length, true)} disabled={loading}>加载更多</button>}
    </section>;
  }

  return <>
  <section className="artifact-review-workspace" aria-labelledby="artifact-review-title">
    <header className="artifact-review-header">
      <div><span className="section-label">Artifact Review</span><h2 id="artifact-review-title">产物审核队列</h2><p>{page ? `${page.total} 个待审核产物 · 项目范围：${projectId}` : "正在加载审核队列…"}</p></div>
      <button type="button" className="quiet-button" onClick={() => void load()} disabled={loading}>刷新队列</button>
    </header>
    {error && <p role="alert" className="artifact-review-error">{error}</p>}
    {loading && !page && <p role="status">正在加载产物…</p>}
    {items.length === 0 && !loading && !error && <p className="artifact-review-empty" role="status">没有待审核产物。新生成的每个文件会作为独立产物进入此队列。</p>}
    {items.length > 0 && <div className="artifact-review-layout">
      <aside className="artifact-review-queue" aria-label="待审核产物列表">
        <ol>
          {items.map((item) => <li key={item.artifact.id}>
            <button type="button" className={selectedArtifactId === item.artifact.id ? "is-selected" : ""} aria-pressed={selectedArtifactId === item.artifact.id} onClick={() => { selectedArtifactIdRef.current = item.artifact.id; setSelectedArtifactId(item.artifact.id); setComment(item.review.comment); setError(undefined); }}>
              <span>{item.artifact.name}</span><ReviewBadge decision={item.review.decision} />
              <small>任务 {item.task?.id ?? item.artifact.taskId} · {item.artifact.mediaType.toUpperCase()}</small>
              {item.artifact.availability !== "available" && <small className="artifact-missing-label">{availabilityLabel(item.artifact.availability)}</small>}
            </button>
          </li>)}
        </ol>
        {page && items.length < page.total && <button type="button" className="quiet-button" onClick={() => void load(items.length, true)} disabled={loading}>加载更多</button>}
      </aside>
      {selected ? <>
        <main className="artifact-review-main">
          <div className="artifact-review-selected-heading"><div><h3>{selected.artifact.name}</h3><ReviewBadge decision={selected.review.decision} /></div><ArtifactActions artifact={selected.artifact} /></div>
          <ArtifactPreview projectId={projectId} artifact={selected.artifact} />
          <ArtifactMetadata artifact={selected.artifact} />
          {selected.review.comment && <p className="artifact-review-existing-comment"><strong>当前备注：</strong>{selected.review.comment}</p>}
        </main>
        <aside className="artifact-review-context" aria-label="任务与审核信息">
          <h3>生成上下文</h3>
          <dl className="artifact-metadata">
            <div><dt>任务</dt><dd>{selected.task?.id ?? selected.artifact.taskId}</dd></div>
            <div><dt>执行状态</dt><dd>{selected.task?.status ?? "未知"}</dd></div>
            <div><dt>工作流版本</dt><dd>{selected.task?.workflowVersionId ?? "未知"}</dd></div>
            <div><dt>配方</dt><dd>{selected.task?.recipeId ?? "未知"}</dd></div>
            <div><dt>审核修订</dt><dd>{selected.review.revision}</dd></div>
          </dl>
          <label className="artifact-review-comment-label" htmlFor="artifact-review-comment">审核备注{selected.artifact.availability === "available" ? "（驳回时必填）" : ""}</label>
          <textarea id="artifact-review-comment" value={comment} onChange={(event) => { setComment(event.target.value); setError(undefined); }} rows={5} maxLength={4000} placeholder="记录通过说明或驳回原因…" disabled={submitting} />
          <div className="artifact-review-decisions">
            <button type="button" className="primary" onClick={() => void submit("APPROVED")} disabled={submitting || selected.artifact.availability !== "available"}>{submitting ? "提交中…" : "通过"}</button>
            <button type="button" className="danger" onClick={() => void submit("REJECTED")} disabled={submitting || selected.artifact.availability !== "available" || !comment.trim()}>{submitting ? "提交中…" : "驳回"}</button>
          </div>
          {selected.artifact.availability !== "available" && <p className="artifact-review-error" role="status">{availabilityLabel(selected.artifact.availability)}；文件可用前不能审核。</p>}
        </aside>
      </> : <p className="artifact-review-empty" role="status">当前页没有可选择的待审核产物。</p>}
    </div>}
  </section>
  <ReviewHistory projectId={projectId} refreshKey={historyRefreshKey} />
  </>;
}

function ReviewHistory({ projectId, refreshKey }: { projectId: string; refreshKey: number }) {
  const [page, setPage] = useState<ArtifactReviewQueuePageDto>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();

  const load = useCallback(async (offset = 0, append = false) => {
    setLoading(true);
    setError(undefined);
    try {
      const next = await getArtifactReviewQueue(projectId, WORKSPACE_PAGE_SIZE, offset, "completed");
      setPage((current) => append && current ? { ...next, items: [...current.items, ...next.items] } : next);
    } catch (cause: unknown) {
      setError(toUserMessage(cause));
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => {
    setPage(undefined);
    void load();
  }, [load, refreshKey]);

  const items = page?.items ?? [];
  return <section className="artifact-review-history" aria-labelledby="artifact-review-history-title">
    <header className="artifact-review-history-header">
      <div><span className="section-label">Review History</span><h3 id="artifact-review-history-title">已完成审核</h3><p>{page ? `${page.total} 条已完成审核` : "正在读取审核历史…"}</p></div>
      <button type="button" className="quiet-button" onClick={() => void load()} disabled={loading}>刷新历史</button>
    </header>
    {error && <p role="alert" className="artifact-review-error">{error}</p>}
    {loading && !page && <p role="status">正在读取审核历史…</p>}
    {page && items.length === 0 && !loading && <p className="artifact-review-empty" role="status">暂无已完成审核记录。</p>}
    <div className="artifact-review-history-list">
      {items.map((item) => <article key={item.artifact.id} className="artifact-review-history-item" data-testid={`artifact-review-history-${item.artifact.id}`}>
        <header><div><strong>{item.artifact.name}</strong><ReviewBadge decision={item.review.decision} /></div></header>
        <p className="artifact-review-history-task">任务 {item.task?.id ?? item.artifact.taskId} · {item.task?.status ?? "状态未知"}</p>
        <ArtifactMetadata artifact={item.artifact} />
        <p className="artifact-review-history-comment"><strong>审核备注：</strong>{item.review.comment || "无备注"}</p>
        <dl className="artifact-review-history-revision">
          <div><dt>Revision</dt><dd>{item.review.revision}</dd></div>
          <div><dt>审核时间</dt><dd>{formatDateTime(item.review.updatedAt)}</dd></div>
        </dl>
      </article>)}
    </div>
    {page && items.length < page.total && <button type="button" className="quiet-button" onClick={() => void load(items.length, true)} disabled={loading}>加载更多历史</button>}
  </section>;
}

function SummaryItem({ projectId, item }: { projectId: string; item: ArtifactReviewQueueItemDto }) {
  return <article className="production-review-inbox-row">
    <div className="production-review-inbox-copy"><strong>{item.artifact.name}</strong><span>任务 {item.task?.id ?? item.artifact.taskId} · {item.artifact.mediaType.toUpperCase()} · v{item.artifact.version}</span><ReviewBadge decision={item.review.decision} />
      {item.artifact.availability !== "available" && <small className="artifact-missing-label">{availabilityLabel(item.artifact.availability)}</small>}
    </div>
    <ArtifactActions artifact={item.artifact} />
    <ArtifactPreview projectId={projectId} artifact={item.artifact} />
  </article>;
}

function availabilityLabel(value: ArtifactReviewQueueItemDto["artifact"]["availability"]): string {
  if (value === "missing") return "文件不存在";
  if (value === "unavailable") return "文件位置不可用";
  return "可用";
}
