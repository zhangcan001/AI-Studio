import { useCallback, useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { RunResult } from "../../product/types";
import type { StoryboardShot } from "./storyboardModel";
import { useStudioStore } from "../../stores/studioStore";
import { CandidateThumbnail } from "./CreateMediaPreview";

const labels: Record<string, string> = { PENDING: "待审核", APPROVED: "已通过", REJECTED: "已退回" };
export function ShotReviewDialog({ projectId, item, onClose, onChanged, onNext, onEdit }: {
  projectId: string; item: StoryboardShot; onClose: () => void; onChanged: () => void;
  onNext?: () => void | string | Promise<void | string>; onEdit: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [results, setResults] = useState<RunResult[]>();
  const [focusedId, setFocusedId] = useState<string>();
  const [autoNext, setAutoNext] = useState(false);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const lock = useRef(false), epoch = useRef(0);
  const [navigationNote,setNavigationNote]=useState<string>();
  const comments = useStudioStore(state => state.reviewComments);
  const commentKey = (assetId: string) => JSON.stringify([projectId, item.run?.ref.source, item.run?.ref.id, assetId]);
  const hasDraft = () => {
    const current = useStudioStore.getState().reviewComments;
    return item.candidates.some(asset => current[commentKey(asset.id)]?.trim());
  };
  const runRef = item.run?.ref;
  useEffect(() => {
    const opener = document.activeElement;
    dialog.current?.showModal();
    return () => { if (opener instanceof HTMLElement && opener.isConnected) opener.focus(); };
  }, []);
  const reload = useCallback(async (keepError = false) => {
    if (!runRef) return;
    const token = ++epoch.current;
    try {
      const out = await productClient.run.resultsGet(projectId, runRef);
      if (token === epoch.current) {
        setResults(out.filter(r => item.candidates.some(a => a.id === r.assetId)));
        if (!keepError) setError(undefined);
      }
    } catch (cause) { if (token === epoch.current) { setError(normalizeProductError(cause).message); setResults([]); } }
  }, [projectId, runRef?.source, runRef?.id, item.id, JSON.stringify(item.candidates.map(a => a.id))]);
  useEffect(() => { setResults(undefined); void reload(); return () => { ++epoch.current; }; }, [reload]);
  const leave = (fn: () => void) => {
    if (lock.current || (hasDraft() && !window.confirm("审核说明尚未提交，本次会话会保留草稿。确定离开？"))) return;
    fn();
  };
  const act = async (
    fn: () => Promise<unknown>,
    submitted?: { key: string; comment: string },
    partialCommitted?: () => boolean,
  ) => {
    if (lock.current) return false;
    lock.current = true; setBusy(true); const token = epoch.current;
    try {
      await fn();
      if (token !== epoch.current) return false;
      if (submitted && useStudioStore.getState().reviewComments[submitted.key] === submitted.comment) useStudioStore.getState().setReviewComment(submitted.key, undefined);
      const refreshing = reload();
      const refreshToken = epoch.current;
      await refreshing;
      if (refreshToken !== epoch.current) return false;
      onChanged();
      return true;
    } catch (cause) {
      if (token === epoch.current) setError(normalizeProductError(cause).message);
      if (partialCommitted?.() && token === epoch.current) {
        if (submitted && useStudioStore.getState().reviewComments[submitted.key] === submitted.comment) {
          useStudioStore.getState().setReviewComment(submitted.key, undefined);
        }
        const refreshing = reload(true);
        const refreshToken = epoch.current;
        await refreshing;
        if (refreshToken === epoch.current) onChanged();
      }
      return false;
    }
    finally { lock.current = false; setBusy(false); }
  };
  const next = async()=>{
    if(!onNext||lock.current||(hasDraft()&&!window.confirm("审核说明尚未提交，本次会话会保留草稿。确定离开？")))return;
    lock.current=true;setBusy(true);const token=epoch.current;setNavigationNote(undefined);
    try{const message=await onNext();if(token===epoch.current&&message)setNavigationNote(message);}
    catch(cause){if(token===epoch.current)setError(normalizeProductError(cause).message);}
    finally{lock.current=false;if(token===epoch.current)setBusy(false);}
  };
  const review = (r: RunResult, decision: "APPROVED" | "REJECTED") => {
    const key = commentKey(r.assetId), comment = comments[key] ?? "";
    return runRef && r.reviewRevision !== null && act(() => productClient.run.resultReview(projectId, { runRef, assetId: r.assetId, decision, comment, expectedRevision: r.reviewRevision! }), { key, comment });
  };
  const focusedResult = results?.find(r => r.assetId === focusedId)
    ?? results?.find(r => r.selectedShotIds.includes(item.id) || item.shot?.selectedResultId === r.assetId)
    ?? results?.[0];
  const stagePlayable = Boolean(focusedResult && focusedResult.mediaKind === "video" && focusedResult.assetExists && focusedResult.availability !== "missing" && focusedResult.availability !== "unavailable");
  return <dialog ref={dialog} className="storyboard-review" aria-label={`审核 ${item.name}`} onCancel={e => { e.preventDefault(); leave(onClose); }} onKeyDown={e => {
    if (
      e.target instanceof HTMLInputElement
      || e.target instanceof HTMLTextAreaElement
      || e.target instanceof HTMLSelectElement
      || e.target instanceof HTMLMediaElement
      || (e.target instanceof HTMLElement && e.target.isContentEditable)
      || busy
    ) return;
    if ((e.key.toLowerCase() === "n" || e.key === "ArrowRight") && onNext) { e.preventDefault(); void next(); }
    else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && results && results.length > 1) {
      e.preventDefault();
      const idx = results.findIndex(r => r.assetId === focusedResult?.assetId);
      const step = e.key === "ArrowDown" ? 1 : -1;
      const target = results[(idx + step + results.length) % results.length];
      if (target) setFocusedId(target.assetId);
    }
  }}>
    <header><h2>审核 · {item.name}</h2><button type="button" disabled={busy} onClick={() => leave(onClose)}>关闭</button></header>
    <p>审核、选用和生成保持独立。退回不会自动重跑；修改仅影响此镜头的新运行。</p>
    <small>未提交说明仅在本次会话内按项目、运行和候选保留；关闭后可恢复。快捷键：↑/↓ 切换候选，N / → 下一待审镜头。</small>
    {error && <p role="alert">{error}</p>}{navigationNote&&<p role="status">{navigationNote}</p>}
    {stagePlayable && focusedResult && <div className="storyboard-review-stage">
      <video key={`${projectId}:${focusedResult.assetId}`} controls playsInline preload="metadata" aria-label={`审核主预览 ${focusedResult.name}`} src={productClient.creation.mediaUrl(projectId, focusedResult.assetId, "video")} />
    </div>}
    {!results ? <p role="status">正在读取候选…</p> : !results.length ? <p>该运行没有可审核的结果；更早候选可在运行记录中审核。</p> :
      <ul className="storyboard-review-list">{results.map(r => {
        const key = commentKey(r.assetId), comment = comments[key] ?? "";
        const asset = item.candidates.find(a => a.id === r.assetId);
        const chosen = r.selectedShotIds.includes(item.id) || item.shot?.selectedResultId === r.assetId;
        const pending = r.reviewState === "PENDING" && r.reviewRevision !== null && r.assetExists && r.availability !== "missing" && r.availability !== "unavailable";
        return <li key={r.assetId} aria-label={`候选 ${r.name}`} data-focused={focusedResult?.assetId === r.assetId || undefined} onClick={() => setFocusedId(r.assetId)}>
          <div className="storyboard-cover">{asset && <CandidateThumbnail asset={asset} playable />}</div>
          <div><strong>{r.name}</strong><small>{r.reviewState ? labels[r.reviewState] : "无审核记录"}{chosen ? " · 已选为结果" : ""}</small></div>
          <label className="storyboard-review-comment" onClick={e => e.stopPropagation()}>审核说明<input value={comment} disabled={busy} onChange={e => useStudioStore.getState().setReviewComment(key, e.target.value)} /></label>
          <div className="storyboard-card-actions" onClick={e => e.stopPropagation()}>
            {pending && <><button type="button" disabled={busy} onClick={() => void review(r, "APPROVED")}>通过</button>{!chosen && <button type="button" className="primary" disabled={busy} onClick={() => {
              const key = commentKey(r.assetId), comment = comments[key] ?? "";
              if (!runRef || r.reviewRevision === null) return;
              let reviewed = false;
              void act(async () => {
                await productClient.run.resultReview(projectId, { runRef, assetId: r.assetId, decision: "APPROVED", comment, expectedRevision: r.reviewRevision! });
                reviewed = true;
                await productClient.creation.selectResult(projectId, item.id, "video", r.assetId);
              }, { key, comment }, () => reviewed).then(saved => { if (saved && autoNext && onNext) void next(); });
            }}>通过并选为结果</button>}<button type="button" disabled={busy || !comment.trim()} onClick={() => void review(r, "REJECTED")}>退回</button></>}
            {r.reviewState && !pending && r.reviewRevision !== null && <button type="button" disabled={busy || (!r.assetExists || r.availability === "missing" || r.availability === "unavailable")} onClick={() => { if (window.confirm("重新打开审核？原审核记录会保留，需要重新明确作出决定。")) void act(() => productClient.run.resultReviewReset(projectId, runRef!, r.assetId, r.reviewRevision!)); }}>重新审核</button>}
            <button type="button" disabled={busy || chosen || !r.assetExists || r.availability === "missing" || r.availability === "unavailable" || r.reviewState === "REJECTED"} onClick={() => void act(() => productClient.creation.selectResult(projectId, item.id, "video", r.assetId)).then(saved => { if (saved && autoNext && onNext) void next(); })}>{chosen ? "当前结果" : "选为结果"}</button>
          </div>
        </li>;
      })}</ul>}
    <footer><button type="button" disabled={busy} onClick={() => leave(onEdit)}>修改此镜头</button>{onNext && <><label className="storyboard-review-autonext"><input type="checkbox" checked={autoNext} onChange={e => setAutoNext(e.target.checked)} /> 选用后自动跳至下一待审镜头</label><button type="button" disabled={busy} onClick={() => void next()}>下一个待审核镜头</button></>}</footer>
  </dialog>;
}
