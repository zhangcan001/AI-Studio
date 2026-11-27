import { useStudioStore } from "../../stores/studioStore";
import { ResultFileActions } from "../../components/ResultFileActions";
import { useEffect, useState } from "react";
import { productClient } from "../../product/client";
import type { RunResult } from "../../product/types";
import type { RunsProps, RunsController } from "./RunsController";

function useBytesObjectUrl(bytes?: number[] | null) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    if (!bytes?.length) { setUrl(undefined); return; }
    const next = URL.createObjectURL(new Blob([Uint8Array.from(bytes)], { type: "image/png" }));
    setUrl(next);
    return () => URL.revokeObjectURL(next);
  }, [bytes]);
  return url;
}

function ResultMedia({ result, projectId }: { result: RunResult; projectId: string }) {
  const posterUrl = useBytesObjectUrl(result.thumbnailBytes);
  const canPlayVideo = result.mediaKind === "video" && result.assetExists && result.availability === "available";
  if (canPlayVideo) {
    return (
      <div className="run-result-media">
        <video controls preload="metadata" poster={posterUrl} src={productClient.creation.mediaUrl(projectId, result.assetId)} />
      </div>
    );
  }
  if (posterUrl) {
    return (
      <div className="run-result-media">
        <img src={posterUrl} alt="生成结果" />
      </div>
    );
  }
  return null;
}

const reviewLabels: Record<string, string> = { PENDING: "待审核", APPROVED: "已通过", REJECTED: "已退回" };

function ReviewActions({ result, controller: c, route, singleShot }: { result: RunResult; controller: RunsController; route: RunsProps["route"]; singleShot?: { id: string; name: string } }) {
  const key = JSON.stringify([route.projectId, route.run?.source, route.run?.id, result.assetId]);
  const comment = useStudioStore(state => state.reviewComments[key] ?? "");
  const setComment = (text: string) => useStudioStore.getState().setReviewComment(key, text);
  const clearSubmittedComment = () => { if (useStudioStore.getState().reviewComments[key] === comment) useStudioStore.getState().setReviewComment(key, undefined); };
  const submit = async (decision: "APPROVED" | "REJECTED") => { if (await c.review(result, decision, comment) === true) clearSubmittedComment(); };
  const submitAndSelect = async () => { if (!singleShot) return; await c.approveAndSelectForShot(result, singleShot.id, comment, clearSubmittedComment); };
  const chosen = Boolean(singleShot && result.selectedShotIds.includes(singleShot.id));
  return (
    <div className="run-review-box">
      <label className="run-review-field">
        <span>审核说明</span>
        <input placeholder="可选；退回时需填写原因" value={comment} onChange={event => setComment(event.target.value)} disabled={c.busy} />
      </label>
      <div className="run-review-actions">
        {singleShot && !chosen && result.mediaKind === "video" && result.availability !== "missing" && result.availability !== "unavailable" && (
          <button type="button" className="primary" disabled={c.busy} onClick={() => void submitAndSelect()}>
            通过并选为「{singleShot.name}」结果
          </button>
        )}
        <button type="button" disabled={c.busy} onClick={() => void submit("APPROVED")}>审核通过</button>
        <button type="button" className="danger" disabled={c.busy || !comment.trim()} onClick={() => void submit("REJECTED")}>审核退回</button>
      </div>
      <small className="run-review-hint">未提交说明仅在本次会话内保留，切换记录后可恢复。</small>
    </div>
  );
}

export function RunResults({ controller: c, route, navigate }: RunsProps & { controller: RunsController }) {
  const videoSources = (c.detail?.detail?.sources ?? []).filter(source => source.stage === "video");
  const seenBatchShots = new Set<string>();
  const batchSelectablePairs: { shotId: string; assetId: string }[] = [];
  for (const result of c.results) {
    const singleShot = result.sourceShotIds?.length === 1
      ? videoSources.find(source => source.id === result.sourceShotIds[0])
      : undefined;
    if (!singleShot || seenBatchShots.has(singleShot.id)) continue;
    const eligible = result.mediaKind === "video" && result.assetExists
      && result.availability !== "missing" && result.availability !== "unavailable"
      && result.reviewState !== "REJECTED";
    if (!eligible) continue;
    seenBatchShots.add(singleShot.id);
    if (!result.selectedShotIds.includes(singleShot.id)) {
      batchSelectablePairs.push({ shotId: singleShot.id, assetId: result.assetId });
    }
  }
  return (
    <section className="run-results-section">
      <header className="run-results-head">
        <h3>生成结果{c.results.length ? `（${c.results.length}）` : ""}</h3>
        <div className="run-inline-actions">
          {batchSelectablePairs.length >= 2 && (
            <button
              type="button"
              className="primary"
              disabled={c.busy}
              title="将本批全部未选用且未退回的镜头视频结果一键设为对应镜头的选用结果，方便直接导出剪辑"
              onClick={() => void c.selectBatchResults(batchSelectablePairs)}
            >
              一键全部选用（{batchSelectablePairs.length} 镜）
            </button>
          )}
          <details className="run-quiet-help">
            <summary>ⓘ 状态说明</summary>
            <p>执行成功、素材存在、审核通过与镜头选用是四个独立事实。</p>
          </details>
        </div>
      </header>
      <div className="run-results">
        {c.results.map(result => {
          const singleShot = result.sourceShotIds?.length === 1
            ? videoSources.find(source => source.id === result.sourceShotIds[0])
            : undefined;
          const canSelectForShot = Boolean(singleShot && result.mediaKind === "video" && result.assetExists && result.availability !== "missing" && result.availability !== "unavailable" && result.reviewState !== "REJECTED");
          const chosen = Boolean(singleShot && result.selectedShotIds.includes(singleShot.id));
          const existsText = result.assetExists ? (result.availability === "missing" ? "素材文件缺失" : "素材存在") : "素材不存在";
          const existsTone = result.assetExists && result.availability !== "missing" ? "ok" : "bad";
          const reviewText = result.reviewState ? (reviewLabels[result.reviewState] ?? "审核状态未知") : "无审核记录";
          const reviewTone = result.reviewState === "APPROVED" ? "ok" : result.reviewState === "REJECTED" ? "bad" : result.reviewState === "PENDING" ? "warn" : "muted";
          const selectedText = result.selectedShotIds.length ? "镜头已选用" : "镜头未选用";
          const selectedTone = result.selectedShotIds.length ? "accent" : "muted";
          return (
            <article className="run-result-card" key={result.assetId}>
              <ResultMedia result={result} projectId={route.projectId} />
              <div className="run-result-meta">
                <strong title={result.name}>{result.name}</strong>
                <div className="run-result-facts">
                  <span className={`run-fact-pill tone-${existsTone}`}>{existsText}</span>
                  <span className={`run-fact-pill tone-${reviewTone}`}>{reviewText}</span>
                  <span className={`run-fact-pill tone-${selectedTone}`}>{selectedText}</span>
                </div>
              </div>
              <div className="run-inline-actions">
                {singleShot && (canSelectForShot || chosen) && (
                  <button type="button" className={chosen ? undefined : "primary"} disabled={c.busy || chosen || !canSelectForShot} onClick={() => void c.selectForShot(singleShot.id, result.assetId)}>
                    {chosen ? `已选为「${singleShot.name}」结果` : `选为「${singleShot.name}」结果`}
                  </button>
                )}
                <button type="button" disabled={!result.assetExists} onClick={() => navigate({ kind: "library", projectId: route.projectId, resource: { kind: "asset", id: result.assetId } })}>
                  在素材库查看
                </button>
                {result.mediaKind === "video" && result.assetExists && <ResultFileActions projectId={route.projectId} assetId={result.assetId} />}
              </div>
              {result.reviewRevision !== null && result.assetExists && (
                result.reviewState === "PENDING"
                  ? <ReviewActions result={result} controller={c} route={route} singleShot={singleShot} />
                  : <div className="run-inline-actions"><button type="button" className="quiet-button" disabled={c.busy} onClick={() => void c.reopenReview(result)}>重新审核</button></div>
              )}
            </article>
          );
        })}
      </div>
      {!c.results.length && <p className="runs-empty-note">暂无结果。不会把任务成功当作素材已存在。</p>}
    </section>
  );
}
