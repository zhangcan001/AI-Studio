import { useCallback, useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { RunResult } from "../../product/types";
import type { StoryboardShot } from "./storyboardModel";
import { CandidateThumbnail } from "./CreateMediaPreview";

const labels: Record<string, string> = { PENDING: "待审核", APPROVED: "已通过", REJECTED: "已退回" };
export function ShotReviewDialog({ projectId, item, onClose, onChanged, onNext, onEdit }: {
  projectId: string; item: StoryboardShot; onClose: () => void; onChanged: () => void;
  onNext?: () => void; onEdit: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [results, setResults] = useState<RunResult[]>();
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const lock = useRef(false), epoch = useRef(0);
  const [comment, setComment] = useState("");
  const runRef = item.run?.ref;
  useEffect(() => {
    const opener = document.activeElement;
    dialog.current?.showModal();
    return () => { if (opener instanceof HTMLElement && opener.isConnected) opener.focus(); };
  }, []);
  const reload = useCallback(async () => {
    if (!runRef) return;
    const token = ++epoch.current;
    try {
      const out = await productClient.run.resultsGet(projectId, runRef);
      if (token === epoch.current) { setResults(out.filter(r => item.candidates.some(a => a.id === r.assetId))); setError(undefined); }
    } catch (cause) { if (token === epoch.current) { setError(normalizeProductError(cause).message); setResults([]); } }
  }, [projectId, runRef?.source, runRef?.id, item.id]);
  useEffect(() => { setResults(undefined); void reload(); return () => { ++epoch.current; }; }, [reload]);
  const leave = (fn: () => void) => {
    if (lock.current || (comment.trim() && !window.confirm("审核说明尚未提交，确定放弃并离开？"))) return;
    fn();
  };
  const act = async (fn: () => Promise<unknown>, submittedComment = false) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); const token = epoch.current;
    try { await fn(); if (token !== epoch.current) return; if (submittedComment) setComment(""); await reload(); onChanged(); }
    catch (cause) { if (token === epoch.current) setError(normalizeProductError(cause).message); }
    finally { lock.current = false; setBusy(false); }
  };
  const review = (r: RunResult, decision: "APPROVED" | "REJECTED") => runRef && r.reviewRevision !== null && act(() => productClient.run.resultReview(projectId, { runRef, assetId: r.assetId, decision, comment, expectedRevision: r.reviewRevision! }), true);
  return <dialog ref={dialog} className="storyboard-review" aria-label={`审核 ${item.name}`} onCancel={e => { e.preventDefault(); leave(onClose); }}>
    <header><h2>审核 · {item.name}</h2><button type="button" disabled={busy} onClick={() => leave(onClose)}>关闭</button></header>
    <p>审核、选用和生成保持独立。退回不会自动重跑；修改仅影响此镜头的新运行。</p>
    <label className="storyboard-review-comment">审核说明<input value={comment} disabled={busy} onChange={e => setComment(e.target.value)} /></label>
    {error && <p role="alert">{error}</p>}
    {!results ? <p role="status">正在读取候选…</p> : !results.length ? <p>该运行没有可审核的结果；更早候选可在运行记录中审核。</p> :
      <ul className="storyboard-review-list">{results.map(r => {
        const asset = item.candidates.find(a => a.id === r.assetId);
        const chosen = r.selectedShotIds.includes(item.id) || item.shot?.selectedResultId === r.assetId;
        const pending = r.reviewState === "PENDING" && r.reviewRevision !== null && r.assetExists;
        return <li key={r.assetId} aria-label={`候选 ${r.name}`}>
          <div className="storyboard-cover">{asset && <CandidateThumbnail asset={asset} playable />}</div>
          <div><strong>{r.name}</strong><small>{r.reviewState ? labels[r.reviewState] : "无审核记录"}{chosen ? " · 已选为结果" : ""}</small></div>
          <div className="storyboard-card-actions">
            {pending && <><button type="button" disabled={busy} onClick={() => void review(r, "APPROVED")}>通过</button><button type="button" disabled={busy || !comment.trim()} onClick={() => void review(r, "REJECTED")}>退回</button></>}
            {r.reviewState && !pending && r.reviewRevision !== null && <button type="button" disabled={busy || !r.assetExists} onClick={() => { if (window.confirm("重新打开审核？原审核记录会保留，需要重新明确作出决定。")) void act(() => productClient.run.resultReviewReset(projectId, runRef!, r.assetId, r.reviewRevision!)); }}>重新审核</button>}
            <button type="button" disabled={busy || chosen || !r.assetExists || r.reviewState === "REJECTED"} onClick={() => void act(() => productClient.creation.selectResult(projectId, item.id, "video", r.assetId))}>{chosen ? "当前结果" : "选为结果"}</button>
          </div>
        </li>;
      })}</ul>}
    <footer><button type="button" disabled={busy} onClick={() => leave(onEdit)}>修改此镜头</button>{onNext && <button type="button" disabled={busy} onClick={() => leave(onNext)}>下一个待选用镜头</button>}</footer>
  </dialog>;
}
