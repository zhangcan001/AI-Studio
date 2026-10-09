import { useEffect, useState } from "react";
import type { CreateController } from "./CreateController";
import type { CreateRoute } from "./createModel";
import { CandidateThumbnail, CreateMediaPreview } from "./CreateMediaPreview";
import { RunStatusCard } from "./CreateResults";
import { modeLabel } from "../../product/generatorPresentation";

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
  return <>
    <div className="create-preview-column">
      <section className="create-video-stage" aria-label="视频预览">
        <header><h2>视频预览</h2><span className="create-state-pill">{current ? current.selected ? "已选用结果" : "候选预览" : "等待生成"}</span></header>
        <div className="create-video-screen">{current
          ? <CreateMediaPreview key={`${route.projectId}:${current.id}:${previewAttempt}`} projectId={route.projectId} asset={current} onUnavailable={() => setUnavailable(previous => new Set([...previous,current.id]))} />
          : <div className="create-video-empty"><span aria-hidden="true">▷</span><h3>让创意开始流动</h3><p>填写左侧提示词与视频输入，生成后在这里预览真实结果。</p><small>没有视频时不显示演示素材或模拟进度。</small></div>}
        </div>
        {current && <div className="create-preview-actions"><div><strong>{current.name}</strong><small>{current.selected ? "已选用；审核状态请在运行详情查看。" : "预览不会选用；选用不等于审核。"}</small></div><button type="button" disabled={c.busy || current.selected || unavailable.has(current.id)} onClick={() => void c.selectResult(current.id)}>选用此结果</button></div>}
      </section>
      <section className="create-candidate-strip" aria-label="当前镜头候选"><header><h2>当前镜头候选 <small>{candidates.length} 个视频</small></h2><div><button type="button" disabled={c.busy} onClick={() => { void c.refresh(); setUnavailable(new Set()); setPreviewAttempt(n => n+1); }}>刷新结果</button><button type="button" aria-label="在资源库查看视频" onClick={() => c.openLibrary("videos")}>资源库</button></div></header>
        {c.run?.status === "FAILED" && <p>本次生成失败；已有候选和选用状态不会删除。</p>}
        {!candidates.length && <p>尚无此阶段的生成候选。</p>}
        <div className="create-video-candidates">{candidates.map(asset => <button key={asset.id} type="button" className={`create-candidate-card ${current?.id === asset.id ? "previewing" : ""}`} aria-label={`预览${asset.name}`} aria-pressed={current?.id === asset.id} onClick={() => setPreviewId(asset.id)}><CandidateThumbnail asset={asset} /><strong>{asset.name}</strong><span>{asset.selected ? "已选用" : "候选"}</span></button>)}</div>
        <small>切换仅预览 · 选用不等于审核 · 其他候选和历史运行保留</small>
      </section>
    </div>
    <aside className="create-run-inspector" aria-label="镜头与运行信息"><details open={inspectorOpen} onToggle={e => setInspectorOpen(e.currentTarget.open)}><summary>镜头与运行信息</summary><section><h2>当前镜头</h2><strong>{c.context?.selectedShot?.summary.name}</strong><p>MiniMax 视频 · {c.generator ? modeLabel(c.generator) : "未选择模式"}</p><small>当前 Recipe 的草稿与原运行快照独立。</small></section><RunStatusCard controller={c} /></details></aside>
  </>;
}
