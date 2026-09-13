import { useCallback, useEffect, useState } from "react";
import { getProductionReviewInbox } from "../../services/tauriClient";
import type { ProductionReviewInboxPage } from "../../types/productionItemReview";
import { formatDateTime, productionReviewStatusLabel } from "../../i18n/statusLabels";
import type { ProjectCommandCenterNavigationRequest } from "../projects/ProjectCommandCenter";

const PAGE_SIZE = 25;

interface Props {
  projectId?: string;
  onNavigate?: (request: ProjectCommandCenterNavigationRequest) => void;
  mode?: "summary" | "workspace";
}

export function ProductionReviewInbox({ projectId, onNavigate, mode = "summary" }: Props) {
  const [page, setPage] = useState<ProductionReviewInboxPage>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();

  const load = useCallback(async (offset: number, append: boolean) => {
    if (!projectId) return;
    setLoading(true);
    setError(undefined);
    try {
      const next = await getProductionReviewInbox(projectId, PAGE_SIZE, offset);
      setPage((current) => append && current ? { ...next, items: [...current.items, ...next.items] } : next);
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : "项目审片收件箱加载失败");
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => {
    setPage(undefined);
    void load(0, false);
  }, [load]);

  if (!projectId) return null;
  const items = page?.items ?? [];
  const navigate = (request: ProjectCommandCenterNavigationRequest) => onNavigate?.({ ...request, projectId });
  return (
    <section className={`project-command-card production-review-inbox${mode === "workspace" ? " production-review-inbox-workspace" : ""}`} aria-labelledby="production-review-inbox-title">
      <div className="project-command-card-heading">
        <div>
          <span className="section-label">项目级审片</span>
          <h3 id="production-review-inbox-title">{mode === "workspace" ? "完整待审核集合" : "待审核结果"}</h3>
          <p>{page ? `${page.total} 项待处理 · 待审核 ${page.unreviewedCount} · 待返工 ${page.regenerateCount}` : "正在加载待处理结果…"}</p>
          <small className="production-review-inbox-filter" role="status">筛选：待审核 / 待返工 · 项目范围：{projectId}</small>
        </div>
        <div className="production-review-inbox-heading-actions">
          {page && page.total > page.items.length && mode === "summary" && <button type="button" className="quiet-button" onClick={() => navigate({ destination: "shots", section: "review", collectionFilter: { kind: "review", state: "PENDING" } })}>查看全部 {page.total}</button>}
          <button type="button" className="quiet-button" onClick={() => void load(0, false)} disabled={loading}>刷新</button>
        </div>
      </div>
      {error && <p className="error-message" role="alert">{error}</p>}
      {items.length === 0 && !loading && !error && <p className="disabled-note">当前项目没有待处理审片项。完成生产后，待审核结果会出现在这里；已选择结果可从项目中心的“最终结果”进入。</p>}
      {items.length > 0 && (
        <div className="production-review-inbox-list">
          {items.map((item) => (
            <article className="production-review-inbox-row" key={`${item.batchId}:${item.itemId}`}>
              <div className="production-review-inbox-copy">
                <strong>{item.promptSummary || `第 ${item.ordinal + 1} 项`}</strong>
                <span>{item.batchName} · #{item.ordinal + 1} · {productionReviewStatusLabel(item.reviewStatus)}</span>
                {item.selectedAssetId && <span>已选择结果 · 可打开素材查看</span>}
                <details><summary>查看技术详情</summary><small>
                  批次 {item.batchId} · 项目 {item.itemId}
                  {item.shotId ? ` · 镜头 ${item.shotId}` : ""}
                  {item.taskId ? ` · 任务 ${item.taskId}` : " · 尚未创建任务"}
                  {item.assetId ? ` · 输出 ${item.assetName || item.assetId}` : " · 暂无输出资产"}
                  {item.selectedAssetId ? ` · 镜头已选 ${item.selectedAssetId}` : ""}
                </small>
                <small>更新于 {formatDateTime(item.updatedAt)} · {item.workflowVersionId} · {item.recipeId}</small></details>
              </div>
              <div className="production-review-inbox-actions">
                <button type="button" className="quiet-button" onClick={() => navigate({ destination: "shots", section: "review", batchId: item.batchId, itemId: item.itemId, reviewId: item.itemId, taskId: item.taskId, shotId: item.shotId, stage: item.stage })} disabled={!onNavigate}>打开审片</button>
                {item.taskId && <button type="button" className="quiet-button" onClick={() => navigate({ destination: "tasks", taskId: item.taskId })} disabled={!onNavigate}>任务</button>}
                {item.shotId && <button type="button" className="quiet-button" onClick={() => navigate({ destination: "shots", section: "creation", shotId: item.shotId })} disabled={!onNavigate}>镜头</button>}
                {item.assetId && <button type="button" className="quiet-button" onClick={() => navigate({ destination: "assets", assetId: item.assetId })} disabled={!onNavigate}>资产</button>}
                {item.selectedAssetId && <button type="button" className="quiet-button" onClick={() => navigate({ destination: "assets", assetId: item.selectedAssetId })} disabled={!onNavigate}>已选择结果</button>}
              </div>
            </article>
          ))}
        </div>
      )}
      {page && page.items.length < page.total && (
        <button type="button" className="quiet-button" onClick={() => void load(page.items.length, true)} disabled={loading}>加载更多</button>
      )}
    </section>
  );
}
