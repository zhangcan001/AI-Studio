import { useEffect, useState } from "react";
import { productClient } from "../../product/client";
import type { ProductRun } from "../../product/types";
import { queueRunLabel } from "./createModel";
import type { CreateController } from "./CreateController";
import type { CreateRoute } from "./createModel";
import { AssetImageThumb, CandidateThumbnail, CreateMediaPreview } from "./CreateMediaPreview";
import { RunStatusCard } from "./CreateResults";
import { modeLabel } from "../../product/generatorPresentation";

const emptyRunMessages: Record<ProductRun["status"], string> = {
  RUNNING: "视频正在生成，请等待真实结果。可打开运行查看详情，无需重复提交。",
  QUEUED: "任务尚在队列中，请在运行详情查看实际启动状态。",
  PAUSED: "运行已暂停或暂时无法启动，请打开运行查看原因。",
  FAILED: "本次运行未完成，可在运行详情查看状态；不会自动重试。",
  CANCELLED: "本次运行未完成，可在运行详情查看状态；不会自动重试。",
  SUCCEEDED: "运行已结束，暂未读到可预览的视频。请刷新结果或打开运行查看详情。",
  PARTIAL: "运行已结束，暂未读到可预览的视频。请刷新结果或打开运行查看详情。",
};

export function CreateVideoWorkspace({ controller: c, route }: { controller: CreateController; route: CreateRoute }) {
  // This component is keyed by project/Shot/Recipe. Preview is not Studio selection.
  const candidates = c.context?.candidates.filter(a => a.mediaKind === "video") ?? [];
  const [previewId, setPreviewId] = useState<string>();
  const [unavailable, setUnavailable] = useState(new Set<string>());
  const [previewAttempt, setPreviewAttempt] = useState(0);
  const [inspectorOpen, setInspectorOpen] = useState(() => typeof window === "undefined" || !window.matchMedia?.("(max-width: 1100px)").matches);
  useEffect(() => {
    const query = window.matchMedia?.("(max-width: 1100px)");
    const resize = () => setInspectorOpen(!query?.matches);
    query?.addEventListener("change", resize);
    return () => query?.removeEventListener("change", resize);
  }, []);
  const current = candidates.find(a => a.id === previewId)
    ?? candidates.find(a => a.selected) ?? candidates[0];
  // Execution comes from the existing RunRef projection, not candidate presence
  // or readiness. An acceptance receipt alone must never claim GPU completion.
  const runLabel = c.run ? c.run.status === "RUNNING" ? "生成中" : queueRunLabel(c.run)
    : c.accepted?.accepted ? c.accepted.startOutcome === "FAILED_TO_START" ? "启动失败"
      : c.accepted.startOutcome === "QUEUED" ? "已入队，正在读取运行状态" : "已提交，正在读取运行状态"
    : c.runRef ? "正在读取运行状态"
    : undefined;
  const runMessage = c.run ? emptyRunMessages[c.run.status]
    : c.accepted?.accepted ? "运行已保存，请在运行详情查看正式状态；接收回执不代表生成成功。"
    : c.runRef ? "正在读取此运行的正式状态，请在运行详情查看；不会自动重复提交。"
    : "填写左侧提示词与视频输入，生成后在这里预览真实结果。";
  return <>
    <div className="create-preview-column">
      <section className="create-video-stage" aria-label="视频预览">
        <header><h2>视频预览</h2><span className="create-state-pill" role={current ? undefined : "status"} aria-label={current ? undefined : "视频生成状态"}>{current ? current.selected ? "已选用结果" : "候选预览" : runLabel ?? "等待生成"}</span></header>
        {current && runLabel && <p role="status" aria-label="视频生成状态">{runLabel}</p>}
        <div className="create-video-screen">{current
          ? <CreateMediaPreview key={`${route.projectId}:${current.id}:${previewAttempt}`} projectId={route.projectId} asset={current} onUnavailable={() => setUnavailable(previous => new Set([...previous,current.id]))} />
          : <div className="create-video-empty"><span aria-hidden="true">▷</span><h3>{c.run?.status === "RUNNING" ? "正在生成视频" : runLabel ? "查看本次运行" : "让创意开始流动"}</h3><p>{runMessage}</p><small>没有视频时不显示演示素材或模拟进度。</small></div>}
        </div>
        {current && <div className="create-preview-actions"><div><strong>{current.name}</strong><small>{current.selected ? "已选用；审核状态请在运行详情查看。" : "预览不会选用；选用不等于审核。"}</small></div><button type="button" disabled={c.busy || current.selected || unavailable.has(current.id)} onClick={() => void c.selectResult(current.id)}>选用此结果</button></div>}
        <div className="create-action-row" aria-label="结果操作">
          <button type="button" disabled={c.busy || !c.run?.availableActions.includes("RETRY")} title="使用原运行快照重新生成" onClick={() => void c.retry()}>重新生成</button>
          <button type="button" disabled={c.busy || !c.generator?.availability || !c.videoInputs.ready || c.readinessState !== "READY"} title="按当前草稿新建一次运行" onClick={() => void c.generate()}>继续生成</button>
          <button type="button" onClick={() => c.openLibrary("videos")}>在资源库查看</button>
          <button type="button" className="primary" disabled={!c.runRef} onClick={c.openRun}>打开运行</button>
        </div>
      </section>
      <section className="create-candidate-strip" aria-label="当前镜头候选"><header><h2>当前镜头候选 <small>{candidates.length} 个视频</small></h2><div><button type="button" disabled={c.busy} onClick={() => { void c.refresh(); setUnavailable(new Set()); setPreviewAttempt(n => n+1); }}>刷新结果</button><button type="button" aria-label="在资源库查看视频" onClick={() => c.openLibrary("videos")}>资源库</button></div></header>
        {c.run?.status === "FAILED" && <p>本次生成失败；已有候选和选用状态不会删除。</p>}
        {!candidates.length && <p>尚无此阶段的生成候选。</p>}
        <div className="create-video-candidates">{candidates.map(asset => <button key={asset.id} type="button" className={`create-candidate-card ${current?.id === asset.id ? "previewing" : ""}`} aria-label={`预览${asset.name}`} aria-pressed={current?.id === asset.id} onClick={() => setPreviewId(asset.id)}><CandidateThumbnail asset={asset} projectId={route.projectId} /><strong>{asset.name}</strong><span>{asset.selected ? "已选用" : "候选"}</span></button>)}</div>
        <small>切换仅预览 · 选用不等于审核 · 其他候选和历史运行保留</small>
      </section>
    </div>
    <aside className="create-run-inspector" aria-label="镜头与运行信息"><details open={inspectorOpen} onToggle={e => setInspectorOpen(e.currentTarget.open)}><summary>镜头与运行信息</summary>
      <ShotInfoCard controller={c} candidates={candidates} />
      <section className="create-recent" aria-label="最近生成"><header><h2>最近生成</h2><button type="button" className="create-link" onClick={() => c.openLibrary("videos")}>查看更多</button></header>
        {candidates.length ? <div className="create-recent-grid">{candidates.slice(0, 3).map(asset => <button key={asset.id} type="button" aria-label={`最近生成${asset.name}`} className={current?.id === asset.id ? "previewing" : ""} onClick={() => setPreviewId(asset.id)}><CandidateThumbnail asset={asset} projectId={route.projectId} /></button>)}</div> : <p className="create-muted">尚无生成结果</p>}
      </section>
      <RunStatusCard controller={c} />
      <RunQueueCard controller={c} projectId={route.projectId} />
    </details></aside>
  </>;
}

function ShotInfoCard({ controller: c, candidates }: { controller: CreateController; candidates: { id: string }[] }) {
  const shot = c.context?.selectedShot;
  const num = (key: string) => { const v = c.values[key]; return v?.type === "integer" || v?.type === "number" ? v.value : undefined; };
  const duration = num("duration_seconds"); const w = num("width"), h = num("height");
  return <section className="create-shot-info" aria-label="当前镜头信息"><h2>当前镜头信息</h2><strong>{shot?.summary.name}</strong>
    <p className="create-shot-prompt">{shot?.prompt.trim() || "尚未保存镜头提示词"}</p>
    <dl>
      <div><dt>镜头时长</dt><dd>{duration ? `${duration} 秒` : "未设置"}</dd></div>
      <div><dt>画面尺寸</dt><dd>{w && h ? `${w} × ${h}` : "未设置"}</dd></div>
      <div><dt>生成模式</dt><dd>{c.generator ? `MiniMax 视频 · ${modeLabel(c.generator)}` : "未选择模式"}</dd></div>
      <div><dt>参考素材</dt><dd>{shot?.referenceAssetIds.length ? <div className="storyboard-ref-thumbs" aria-label="参考素材缩略图">{shot.referenceAssetIds.map(id => <AssetImageThumb key={id} projectId={c.context!.projectId} assetId={id} name={`参考 ${id.slice(0, 8)}`} size="lg" />)}<small>{shot.referenceAssetIds.length} 张</small></div> : "0 张"}</dd></div>
      <div><dt>生成候选</dt><dd>{candidates.length} 个</dd></div>
    </dl><small>当前 Recipe 的草稿与原运行快照独立。</small></section>;
}
/** Read-only view of active project runs; the Production Queue stays the only executor. */
function RunQueueCard({ controller: c, projectId }: { controller: CreateController; projectId: string }) {
  const [runs, setRuns] = useState<ProductRun[]>();
  const [failed, setFailed] = useState(false);
  const signal = `${c.runRef?.id ?? ""}:${c.run?.status ?? ""}`;
  useEffect(() => {
    let alive = true;
    Promise.resolve().then(() => productClient.run.list(projectId, "active")).then(list => { if (alive) { setRuns(list.items.slice(0, 3)); setFailed(false); } }).catch(() => { if (alive) setFailed(true); });
    return () => { alive = false; };
  }, [projectId, signal]);
  return <section className="create-run-queue" aria-label="运行队列"><header><h2>运行队列 <small>{runs?.length ?? 0}</small></h2><button type="button" className="create-link" onClick={() => c.openRunsList()}>全部任务</button></header>
    {failed ? <p className="create-muted">无法读取运行队列，可在运行页查看。</p> : !runs ? <p className="create-muted">正在读取…</p> : !runs.length ? <p className="create-muted">当前没有进行中的运行</p>
      : <ol>{runs.map(run => { const pct = run.progress.total ? Math.round(((run.progress.succeeded + run.progress.failed + run.progress.cancelled) / run.progress.total) * 100) : 0;
        return <li key={`${run.ref.source}:${run.ref.id}`}><div><strong>{run.title}</strong><span>{queueRunLabel(run)}</span></div><progress max={100} value={pct} aria-label={`${run.title}进度`} /><small>{pct}%</small>{run.status === "PAUSED" && run.errorSummary ? <small className="create-queue-blocked" role="status">{run.errorSummary}</small> : null}</li>; })}</ol>}
  </section>;
}
