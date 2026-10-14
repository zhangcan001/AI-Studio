import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { AppRoute } from "../../app/routes/types";
import type { CreateRoute } from "./createModel";
import { CandidateThumbnail } from "./CreateMediaPreview";
import { coverAsset, deriveStoryboardStatus, runPercent, shotDuration, shotResolution, storyboardStats, storyboardStatusLabels, videoCandidates, type StoryboardShot } from "./storyboardModel";
import "./CreatePage.css";

/** Bounded per-shot reads keep the workbench responsive on large projects. */
export const STORYBOARD_MAX_DETAIL_SHOTS = 60;
const CONCURRENCY = 4;
const POLL_MS = 8000;

interface Props { route: CreateRoute; navigate: (route: AppRoute) => unknown }

/**
 * Storyboard workbench: the primary daily creation surface. It only reads the
 * product facade (creation context + run status); every generation still goes
 * through the single-shot page or the Production Queue.
 */
export function StoryboardWorkbench({ route, navigate }: Props) {
  const [items, setItems] = useState<StoryboardShot[]>();
  const [truncated, setTruncated] = useState(0);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [view, setView] = useState<"grid" | "list">("grid");
  const [selectedId, setSelectedId] = useState<string>();
  const epoch = useRef(0);
  const inFlight = useRef(false);
  const projectId = route.projectId;

  const load = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true; const token = ++epoch.current; setBusy(true);
    try {
      const base = await productClient.creation.get(projectId, null, "video");
      const summaries = [...base.shots].sort((a, b) => a.ordinal - b.ordinal);
      const bounded = summaries.slice(0, STORYBOARD_MAX_DETAIL_SHOTS);
      const result: StoryboardShot[] = bounded.map(s => ({ id: s.id, name: s.name, ordinal: s.ordinal, shot: null, candidates: [], run: null, status: "TODO" }));
      let next = 0;
      const worker = async () => {
        while (next < bounded.length) {
          const index = next++; const summary = bounded[index];
          try {
            const context = await productClient.creation.get(projectId, summary.id, "video");
            const shot = context.selectedShot; const candidates = videoCandidates(context);
            const run = shot?.recentRun ? await productClient.run.get(projectId, shot.recentRun).catch(() => null) : null;
            result[index] = { ...result[index], shot, candidates, run, status: deriveStoryboardStatus(shot, candidates, run) };
          } catch (cause) { result[index] = { ...result[index], error: normalizeProductError(cause).message }; }
        }
      };
      await Promise.all(Array.from({ length: Math.min(CONCURRENCY, bounded.length) }, worker));
      if (token !== epoch.current) return;
      setItems(result); setTruncated(Math.max(0, summaries.length - bounded.length)); setError(undefined);
    } catch (cause) { if (token === epoch.current) setError(normalizeProductError(cause).message); }
    finally { inFlight.current = false; if (token === epoch.current) setBusy(false); }
  }, [projectId]);

  useEffect(() => { void load(); return () => { epoch.current++; inFlight.current = false; }; }, [load]);
  const anyGenerating = items?.some(item => item.status === "GENERATING") ?? false;
  useEffect(() => {
    if (!anyGenerating) return;
    const timer = window.setInterval(() => void load(), POLL_MS);
    return () => window.clearInterval(timer);
  }, [anyGenerating, load]);

  const stats = useMemo(() => storyboardStats(items ?? []), [items]);
  const selected = items?.find(item => item.id === selectedId) ?? items?.[0];
  const openShot = (shotId: string) => navigate({ kind: "create", projectId, stage: "video", shotId });
  const openRun = (item: StoryboardShot) => item.run && navigate({ kind: "runs", projectId, run: item.run.ref });
  const createShot = async () => {
    setBusy(true);
    try { const shot = await productClient.creation.createShot(projectId); openShot(shot.id); }
    catch (cause) { setError(normalizeProductError(cause).message); setBusy(false); }
  };

  return <div className="storyboard-page" aria-label="分镜工作台">
    <header className="storyboard-header">
      <div className="storyboard-title"><h1>分镜工作台</h1><p>将剧本拆解为多个镜头，逐镜生成并完成整段视频的制作。</p></div>
      <dl className="storyboard-stats" aria-label="镜头统计">
        <div><dt>镜头总数</dt><dd>{stats.total}</dd></div>
        <div data-tone="success"><dt>已完成</dt><dd>{stats.completed}</dd></div>
        <div data-tone="info"><dt>生成中</dt><dd>{stats.generating}</dd></div>
        <div data-tone="muted"><dt>待制作</dt><dd>{stats.todo}</dd></div>
      </dl>
      <div className="storyboard-header-actions">
        <button type="button" onClick={() => void navigate({ kind: "project-settings", projectId, section: "advanced-production" })}>导入分镜脚本</button>
        <button type="button" className="primary" title="前往批量镜头，由生产队列统一执行" onClick={() => void navigate({ kind: "project-settings", projectId, section: "advanced-shots" })}>批量生成待制作镜头</button>
      </div>
    </header>
    {error && <p role="alert" className="storyboard-error">{error}</p>}
    {!items ? <p role="status" className="storyboard-loading">正在读取分镜…</p> : <div className="storyboard-body">
      <aside className="storyboard-panel storyboard-tree" aria-label="剧集与场景">
        <header><h2>剧集与场景</h2><button type="button" disabled={busy} onClick={() => void createShot()}>+ 新建镜头</button></header>
        <div className="storyboard-tree-group"><span>全部镜头</span><span className="storyboard-count">{stats.total + truncated}</span></div>
        <ol className="storyboard-tree-shots">{items.map(item => <li key={item.id}><button type="button" aria-current={selected?.id === item.id ? "true" : undefined} onClick={() => setSelectedId(item.id)}>
          <span className="storyboard-tree-name"><strong>{item.name}</strong><small>{shotDuration(item.shot)}</small></span><StatusBadge item={item} compact />
        </button></li>)}</ol>
        {!items.length && <p className="storyboard-empty-note">还没有镜头。新建镜头或导入分镜脚本后会显示在这里。</p>}
        <button type="button" className="storyboard-tree-manage" onClick={() => void navigate({ kind: "project-settings", projectId, section: "advanced-shots" })}>管理剧集与场景结构</button>
      </aside>
      <main className="storyboard-panel storyboard-list" aria-label="镜头列表">
        <header><h2>镜头列表 <small>({stats.total})</small></h2><div className="storyboard-toggle" role="group" aria-label="视图模式">
          <button type="button" aria-pressed={view === "list"} onClick={() => setView("list")}>列表模式</button>
          <button type="button" aria-pressed={view === "grid"} onClick={() => setView("grid")}>宫格模式</button>
          <button type="button" disabled={busy} onClick={() => void load()}>刷新</button>
        </div></header>
        {!items.length ? <div className="storyboard-empty"><h3>开始你的第一个镜头</h3><p>分镜工作台只显示真实镜头和真实生成结果，不会显示演示素材。</p><button type="button" className="primary" disabled={busy} onClick={() => void createShot()}>创建第一个镜头</button></div>
          : <div className={`storyboard-cards ${view}`}>{items.map(item => <ShotCard key={item.id} item={item} selected={selected?.id === item.id} onSelect={() => setSelectedId(item.id)} onOpen={() => openShot(item.id)} onRun={() => openRun(item)} />)}</div>}
        {truncated > 0 && <small className="storyboard-truncated">另有 {truncated} 个镜头未在此展开，可在批量镜头中管理。</small>}
      </main>
      <aside className="storyboard-side" aria-label="镜头详情与序列">
        <section className="storyboard-panel storyboard-detail" aria-label="镜头详情"><header><h2>镜头详情</h2>{selected && <button type="button" onClick={() => openShot(selected.id)}>进入创作</button>}</header>
          {selected ? <ShotDetail item={selected} /> : <p className="storyboard-empty-note">选择一个镜头查看详情。</p>}
        </section>
        <section className="storyboard-panel storyboard-sequence" aria-label="镜头序列"><header><h2>镜头序列 <small>{stats.total}</small></h2></header>
          <ol>{items.map((item, index) => <li key={item.id}>{index > 0 && <span aria-hidden="true" className="storyboard-arrow">→</span>}<button type="button" aria-label={`选择${item.name}`} aria-current={selected?.id === item.id ? "true" : undefined} onClick={() => setSelectedId(item.id)}><Cover item={item} /><StatusBadge item={item} compact /></button></li>)}</ol>
          {!items.length && <p className="storyboard-empty-note">暂无镜头序列。</p>}
        </section>
      </aside>
    </div>}
  </div>;
}

function Cover({ item }: { item: StoryboardShot }) {
  const asset = coverAsset(item); const percent = item.status === "GENERATING" ? runPercent(item.run) : null;
  return <div className="storyboard-cover">{asset ? <CandidateThumbnail asset={asset} /> : <span className="storyboard-cover-empty">暂无结果</span>}
    {percent !== null && <span className="storyboard-ring" style={{ ["--p" as string]: `${percent}` }} aria-label={`进度 ${percent}%`}>{percent}%</span>}
  </div>;
}
function StatusBadge({ item, compact }: { item: StoryboardShot; compact?: boolean }) {
  const percent = item.status === "GENERATING" ? runPercent(item.run) : null;
  return <span className={`storyboard-badge ${item.status.toLowerCase()}${compact ? " compact" : ""}`}>{item.error ? "读取失败" : storyboardStatusLabels[item.status]}{percent !== null && !compact ? ` ${percent}%` : ""}</span>;
}
function ShotCard({ item, selected, onSelect, onOpen, onRun }: { item: StoryboardShot; selected: boolean; onSelect: () => void; onOpen: () => void; onRun: () => void }) {
  const prompt = item.shot?.prompt.trim();
  return <article className={`storyboard-card${selected ? " selected" : ""}`} aria-label={item.name} onClick={onSelect}>
    <Cover item={item} />
    <div className="storyboard-card-body">
      <header><strong>{item.name}</strong><StatusBadge item={item} /></header>
      <p className={prompt ? "" : "muted"}>{prompt || "尚未填写提示词"}</p>
      <ul className="storyboard-tags"><li>{shotDuration(item.shot)}</li><li>{shotResolution(item.shot)}</li><li>{item.candidates.length} 个候选</li></ul>
      {item.error && <small role="status">{item.error}</small>}
      <div className="storyboard-card-actions" onClick={e => e.stopPropagation()}>
        <button type="button" onClick={onOpen}>查看</button>
        {item.run && <button type="button" onClick={onRun}>{item.status === "GENERATING" ? "查看运行" : "运行记录"}</button>}
        <button type="button" className={item.status === "GENERATING" ? "primary" : ""} onClick={onOpen}>{item.status === "TODO" || item.status === "FAILED" ? "生成" : "继续创作"}</button>
      </div>
    </div>
  </article>;
}
function ShotDetail({ item }: { item: StoryboardShot }) {
  const cover = coverAsset(item);
  return <div className="storyboard-detail-body">
    <div className="storyboard-detail-head"><Cover item={item} /><div><strong>{item.name}</strong><StatusBadge item={item} /></div></div>
    <dl>
      <div><dt>时长</dt><dd>{shotDuration(item.shot)}</dd></div>
      <div><dt>分辨率</dt><dd>{shotResolution(item.shot)}</dd></div>
      <div><dt>提示词摘要</dt><dd>{item.shot?.prompt.trim() || "尚未填写"}</dd></div>
      <div><dt>参考素材</dt><dd>{item.shot?.referenceAssetIds.length ?? 0} 张</dd></div>
      <div><dt>生成候选</dt><dd>{item.candidates.length} 个</dd></div>
      {item.run && <div><dt>最近运行</dt><dd>{item.run.title} · {item.run.progress.succeeded}/{item.run.progress.total}</dd></div>}
    </dl>
    <div className="storyboard-detail-result"><span>当前结果</span>{cover ? <CandidateThumbnail asset={cover} /> : <p className="storyboard-empty-note">尚无生成结果</p>}</div>
  </div>;
}
