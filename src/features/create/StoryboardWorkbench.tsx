import { ShotEntityChips } from "./ShotEntityChips";
import { StoryboardShotRows } from "./StoryboardShotRows";
import { useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { subscribeRunInvalidation } from "../../product/runInvalidation";
import type { AppRoute } from "../../app/routes/types";
import type { RunResult } from "../../product/types";
import { AssetImageThumb, CandidateThumbnail, MediaLightboxScope, ThumbnailProjectContext } from "./CreateMediaPreview";
import { coverAsset, deriveStoryboardStatus, runPercent, shotDuration, shotResolution, storyboardStats, storyboardStatusLabels, videoCandidates, type StoryboardShot } from "./storyboardModel";
import "./CreatePage.css";
import { CreatePage } from "./CreatePage";
import type { CreateProps } from "./CreateController";
import { useStudioStore } from "../../stores/studioStore";

/** Bounded per-shot reads keep the workbench responsive on large projects. */
export const STORYBOARD_MAX_DETAIL_SHOTS = 60;
const CONCURRENCY = 4;
const POLL_MS = 8000;
const EVENT_REFRESH_MS = 500;

interface Props extends CreateProps {
  // Cross-feature composition belongs to the app, not the Create feature.
  renderRuns?: (route: Extract<AppRoute, { kind: "runs" }>, navigate: CreateProps["navigate"]) => ReactNode;
}

/**
 * Storyboard workbench: the primary daily creation surface. It only reads the
 * product facade (creation context + run status). The embedded Create controller
 * retains Studio Store ownership and submits only through the Production Queue.
 */
export function StoryboardWorkbench({ route, navigate, runtime, onDirtyChange, renderRuns }: Props) {
  const [items, setItems] = useState<StoryboardShot[]>();
  const [truncated, setTruncated] = useState(0);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState(false);
  const createInFlight = useRef(false);
  const [view, setViewState] = useState<StoryboardView>(() => readView());
  const setView = (next: StoryboardView) => { setViewState(next); try { window.localStorage.setItem(VIEW_KEY, next); } catch { /* storage unavailable */ } };
  const [reviewId, setReviewId] = useState<string>();
  const [selectedId, setSelectedId] = useState<string>();
  const [runRoute, setRunRoute] = useState<Extract<AppRoute, { kind: "runs" }>>();
  const [shotQuery, setShotQuery] = useState("");
  const libraryIntent = useStudioStore(state => state.pendingLibraryIntent);
  const runIntent = useStudioStore(state => state.pendingRunIntent);
  const matchingRunIntent = runIntent?.projectId === route.projectId && runIntent.stage === route.stage;
  const editing = Boolean(route.shotId || libraryIntent || matchingRunIntent);
  const epoch = useRef(0);
  const inFlight = useRef(false);
  const [queueActive, setQueueActive] = useState(false);
  const mounted = useRef(true);
  const projectId = route.projectId;
  const scope = useRef(projectId);
  scope.current = projectId;
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);

  const load = useCallback(async (background = false) => {
    if (inFlight.current) return;
    inFlight.current = true; const token = ++epoch.current;
    // Progress reads must not toggle every action button. Foreground reads and
    // the existing row/create mutation locks still prevent duplicate actions.
    if (!background) setBusy(true);
    try {
      // Formal Queue/Run read model: a waiting item may have no Task yet, so
      // shot.recentRun alone cannot tell us whether a refresh is still needed.
      const activeRuns = Promise.resolve().then(() => productClient.run.list(projectId, "active", null)).catch(() => null);
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
      const active = await activeRuns;
      if (token !== epoch.current) return;
      setQueueActive(Boolean(active?.items?.length));
      setItems(result); setTruncated(Math.max(0, summaries.length - bounded.length)); setError(undefined);
    } catch (cause) { if (token === epoch.current) setError(normalizeProductError(cause).message); }
    finally { if (token === epoch.current) { inFlight.current = false; setBusy(false); } }
  }, [projectId]);

  useEffect(() => { void load(); return () => { epoch.current++; inFlight.current = false; }; }, [load]);
  const anyGenerating = items?.some(item => item.status === "GENERATING") ?? false;
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let dirty = false;
    const flush = () => {
      timer = undefined;
      if (!dirty) return;
      // Keep the final notification pending while a slow read completes. Do not
      // restart the timer on every event: continuous progress must not starve it.
      if (!inFlight.current) { dirty = false; void load(true); }
      else timer = setTimeout(flush, EVENT_REFRESH_MS);
    };
    const unsubscribe = subscribeRunInvalidation(changed => {
      if (changed !== projectId) return;
      dirty = true;
      if (timer === undefined) timer = setTimeout(flush, EVENT_REFRESH_MS);
    });
    return () => { unsubscribe(); clearTimeout(timer); };
  }, [projectId, load]);
  useEffect(() => {
    if (!anyGenerating && !queueActive) return;
    const timer = window.setInterval(() => void load(true), POLL_MS);
    return () => window.clearInterval(timer);
  }, [anyGenerating, queueActive, load]);

  const stats = useMemo(() => storyboardStats(items ?? []), [items]);
  const selected = items?.find(item => item.id === (route.shotId ?? selectedId)) ?? items?.[0];
  const reviewItem = items?.find(item => item.id === reviewId);
  const openShot = (shotId: string) => navigate({ kind: "create", projectId, stage: "video", shotId });
  const openRun = (item: StoryboardShot) => item.run && setRunRoute({ kind: "runs", projectId, run: item.run.ref });
  const navigateInline = async (next: AppRoute) => {
    if (next.kind === "runs" && next.projectId === projectId) { setRunRoute(next); return true; }
    const store = useStudioStore.getState();
    const intent = store.pendingRunIntent;
    // Same-Shot replacement does not trigger the router's leave-scope guard.
    // Use the existing explicit-discard convention before approving that reuse.
    if (next.kind === "create" && next.projectId === projectId && next.shotId === route.shotId && next.stage === route.stage
      && intent?.target?.approved === false && store.draftDirty
      && !window.confirm("当前镜头有未保存的草稿。用历史运行输入替换该草稿？此操作不会启动生成。")) return false;
    // All scope changes still go through the application's existing draft guard.
    const result = await navigate(next);
    if (result !== false && mounted.current && scope.current === projectId) setRunRoute(undefined);
    return result;
  };
  const createShot = async (open = true) => {
    if (createInFlight.current) return;
    createInFlight.current = true; setCreating(true);
    try {
      const shot = await productClient.creation.createShot(projectId);
      // The authorised creation completes in the background, but a late reply
      // must not pull a user who already left (or switched project) back.
      if (!mounted.current || scope.current !== projectId) return;
      if (open) await openShot(shot.id);
      if (mounted.current && scope.current === projectId) void load();
    } catch (cause) { if (mounted.current && scope.current === projectId) setError(normalizeProductError(cause).message); }
    finally { createInFlight.current = false; if (mounted.current && scope.current === projectId) setCreating(false); }
  };

  return <MediaLightboxScope><ThumbnailProjectContext.Provider value={projectId}><div className="storyboard-page" aria-label="分镜工作台">
    <header className="storyboard-header">
      <div className="storyboard-title"><h1>分镜工作台</h1><p>将剧本拆解为多个镜头，逐镜生成并完成整段视频的制作。</p></div>
      <dl className="storyboard-stats" aria-label="镜头统计">
        <div><dt>镜头总数</dt><dd>{stats.total + truncated}</dd></div>
        <div data-tone="success"><dt>已完成</dt><dd>{stats.completed}</dd></div>
        <div data-tone="info"><dt>生成中</dt><dd>{stats.generating}</dd></div>
        <div data-tone="muted"><dt>待制作</dt><dd>{stats.todo}</dd></div>
      </dl>
      {truncated > 0 && <small role="note">状态统计仅覆盖已加载的 {stats.total} 个镜头。</small>}
      <div className="storyboard-header-actions">
        <button type="button" onClick={() => setRunRoute({ kind: "runs", projectId })}>任务队列与运行记录</button>
        <button type="button" onClick={() => void navigate({ kind: "project-settings", projectId, section: "advanced-production" })}>导入分镜脚本</button>
        <button type="button" className="primary" title="前往批量镜头，由生产队列统一执行" onClick={() => void navigate({ kind: "project-settings", projectId, section: "advanced-shots" })}>批量生成待制作镜头</button>
      </div>
    </header>
    {error && <p role="alert" className="storyboard-error">{error}</p>}
    {editing ? <div className="storyboard-editing-body">
      <aside className="storyboard-panel storyboard-shot-nav" aria-label="镜头切换">
        <header><h2>镜头列表</h2><button type="button" onClick={() => void navigate({ kind: "create", projectId, stage: "video" })}>展开分镜列表</button></header>
        <input type="search" aria-label="搜索操作镜头" placeholder="搜索镜头" value={shotQuery} onChange={e => setShotQuery(e.target.value)} />
        {!items && <p role="status">正在读取分镜…</p>}
        <ol>{items?.filter(item => !shotQuery.trim() || `${item.name} ${item.shot?.prompt ?? ""}`.toLowerCase().includes(shotQuery.trim().toLowerCase())).map(item => <li key={item.id}>
          <button type="button" aria-label={`操作${item.name}`} aria-current={route.shotId === item.id ? "true" : undefined} onClick={() => openShot(item.id)}>
            <Cover item={item} playable={false} /><span><strong>{item.name}</strong><small>{shotDuration(item.shot)} · {shotResolution(item.shot)}</small><StatusBadge item={item} compact /></span>
          </button>
        </li>)}</ol>
        {truncated > 0 && <small>这里只展开前 {STORYBOARD_MAX_DETAIL_SHOTS} 个镜头；全部镜头可从右侧“镜头”选择器访问。</small>}
      </aside>
      <section className="storyboard-inline-editor" aria-label="镜头操作">
        <header><span>提示词 · 参数 · 素材 · 生成 · 候选</span>{route.shotId && selected?.id === route.shotId && canReview(selected) && <button type="button" onClick={() => setReviewId(selected.id)}>人工审核</button>}</header>
        {matchingRunIntent && !route.shotId && <p role="note">请选择目标镜头；历史输入只载入该镜头的未保存草稿，不会自动保存或生成。</p>}
        <section className="storyboard-inline-create" aria-label="镜头内联创作"><CreatePage route={route} runtime={runtime} navigate={navigateInline} onDirtyChange={onDirtyChange} embedded onContextChange={load} /></section>
      </section>
    </div> : !items ? <p role="status" className="storyboard-loading">正在读取分镜…</p> : <div className="storyboard-body">
      <aside className="storyboard-panel storyboard-tree" aria-label="剧集与场景">
        <header><h2>剧集与场景</h2><button type="button" disabled={busy || creating} onClick={() => void createShot()}>+ 新建镜头</button></header>
        <div className="storyboard-tree-group"><span>全部镜头</span><span className="storyboard-count">{stats.total + truncated}</span></div>
        <ol className="storyboard-tree-shots">{items.map(item => <li key={item.id}><button type="button" aria-current={selected?.id === item.id ? "true" : undefined} onClick={() => setSelectedId(item.id)}>
          <span className="storyboard-tree-name"><strong>{item.name}</strong><small>{shotDuration(item.shot)}</small></span><StatusBadge item={item} compact />
        </button></li>)}</ol>
        {!items.length && <p className="storyboard-empty-note">还没有镜头。新建镜头或导入分镜脚本后会显示在这里。</p>}
        <button type="button" className="storyboard-tree-manage" onClick={() => void navigate({ kind: "project-settings", projectId, section: "advanced-shots" })}>管理剧集与场景结构</button>
      </aside>
      <main className="storyboard-panel storyboard-list" aria-label="镜头列表">
        <header><h2>镜头列表 <small>({stats.total})</small></h2><div className="storyboard-toggle" role="group" aria-label="视图模式">
          <button type="button" aria-pressed={view === "shots"} onClick={() => setView("shots")}>分镜模式</button>
          <button type="button" aria-pressed={view === "list"} onClick={() => setView("list")}>列表模式</button>
          <button type="button" aria-pressed={view === "grid"} onClick={() => setView("grid")}>宫格模式</button>
          <button type="button" disabled={busy} onClick={() => void load()}>刷新</button>
        </div></header>
        {!items.length ? <div className="storyboard-empty"><h3>开始你的第一个镜头</h3><p>分镜工作台只显示真实镜头和真实生成结果，不会显示演示素材。</p><button type="button" className="primary" disabled={busy || creating} onClick={() => void createShot()}>创建第一个镜头</button></div>
          : view === "shots" ? <StoryboardShotRows projectId={projectId} items={items} busy={busy || creating} onOpen={openShot} onCreate={() => void createShot(false)} onChanged={() => void load()} onQueue={() => setRunRoute({ kind: "runs", projectId })} />
          : view === "grid" ? <div className="storyboard-cards grid" data-view="grid">{items.map(item => <ShotCard key={item.id} item={item} selected={selected?.id === item.id} onSelect={() => setSelectedId(item.id)} onOpen={() => openShot(item.id)} onRun={() => openRun(item)} onReview={() => setReviewId(item.id)} />)}</div>
          : <div className="storyboard-rows" role="table" aria-label="镜头列表（列表模式）" data-view="list">
            <div role="row" className="storyboard-row head"><span role="columnheader">缩略图</span><span role="columnheader">镜头</span><span role="columnheader">状态</span><span role="columnheader">时长</span><span role="columnheader">分辨率</span><span role="columnheader">候选</span><span role="columnheader">操作</span></div>
            {items.map(item => <ShotRow key={item.id} item={item} selected={selected?.id === item.id} onSelect={() => setSelectedId(item.id)} onOpen={() => openShot(item.id)} onReview={() => setReviewId(item.id)} />)}
          </div>}
        {truncated > 0 && <small className="storyboard-truncated">另有 {truncated} 个镜头未在此展开，可在批量镜头中管理。</small>}
      </main>
      <aside className="storyboard-side" aria-label="镜头详情与序列">
        <section className="storyboard-panel storyboard-detail" aria-label="镜头详情"><header><h2>镜头详情</h2>{selected && <span className="storyboard-detail-actions">{canReview(selected) && <button type="button" onClick={() => setReviewId(selected.id)}>审核</button>}<button type="button" onClick={() => openShot(selected.id)}>设置与生成</button></span>}</header>
          {selected ? <ShotDetail item={selected} /> : <p className="storyboard-empty-note">选择一个镜头查看详情。</p>}
        </section>
        <section className="storyboard-panel storyboard-sequence" aria-label="镜头序列"><header><h2>镜头序列 <small>{stats.total}</small></h2></header>
          <ol>{items.map((item, index) => <li key={item.id}>{index > 0 && <span aria-hidden="true" className="storyboard-arrow">→</span>}<button type="button" aria-label={`选择${item.name}`} aria-current={selected?.id === item.id ? "true" : undefined} onClick={() => setSelectedId(item.id)}><Cover item={item} playable={false} /><StatusBadge item={item} compact /></button></li>)}</ol>
          {!items.length && <p className="storyboard-empty-note">暂无镜头序列。</p>}
        </section>
      </aside>
    </div>}
    {reviewItem && <ReviewPanel key={`${projectId}:${reviewItem.id}:${reviewItem.run?.ref.id}`} projectId={projectId} item={reviewItem} onClose={() => setReviewId(undefined)} onChanged={() => void load()} />}
    {runRoute && <WorkbenchRuns route={runRoute} navigate={navigateInline} renderRuns={renderRuns} onClose={() => setRunRoute(undefined)} />}
  </div></ThumbnailProjectContext.Provider></MediaLightboxScope>;
}

/** Native dialog supplies focus trapping/Escape without unmounting the Create draft. */
function WorkbenchRuns({ route, navigate, renderRuns, onClose }: { route: Extract<AppRoute, { kind: "runs" }>; navigate: Props["navigate"]; renderRuns: Props["renderRuns"]; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  return <dialog ref={dialog} className="storyboard-runs-dialog" aria-label="任务队列与运行记录" onCancel={onClose}>
    <header><strong>任务队列与运行记录</strong><button type="button" onClick={onClose}>返回镜头操作</button></header>
    {renderRuns ? renderRuns(route, navigate) : <p role="alert">运行面板不可用。</p>}
  </dialog>;
}

export type StoryboardView = "shots" | "grid" | "list";
const VIEW_KEY = "ai-studio.storyboard.view";
function readView(): StoryboardView { try { const v = window.localStorage.getItem(VIEW_KEY); return v === "list" || v === "grid" ? v : "shots"; } catch { return "shots"; } }
/** Review needs a real run with results; never offered for shots without candidates. */
export function canReview(item: StoryboardShot) { return Boolean(item.run && item.candidates.length && !item.error); }
const reviewLabels: Record<string, string> = { PENDING: "待审核", APPROVED: "已通过", REJECTED: "已退回" };

/** Review panel: reads the real run results and uses the existing run-result review and creation result-select IPC. */
function ReviewPanel({ projectId, item, onClose, onChanged }: { projectId: string; item: StoryboardShot; onClose: () => void; onChanged: () => void }) {
  const [results, setResults] = useState<RunResult[]>();
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [comment, setComment] = useState("");
  const runRef = item.run?.ref;
  const reload = useCallback(async () => {
    if (!runRef) return;
    try { const out = await productClient.run.resultsGet(projectId, runRef); setResults(out.filter(r => r.mediaKind === "video" || r.mediaKind === null)); setError(undefined); }
    catch (cause) { setError(normalizeProductError(cause).message); setResults([]); }
  }, [projectId, runRef]);
  useEffect(() => { void reload(); }, [reload]);
  const act = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    try { await fn(); await reload(); onChanged(); } catch (cause) { setError(normalizeProductError(cause).message); } finally { setBusy(false); }
  };
  const review = (r: RunResult, decision: "APPROVED" | "REJECTED") => runRef && r.reviewRevision !== null && act(() => productClient.run.resultReview(projectId, { runRef, assetId: r.assetId, decision, comment, expectedRevision: r.reviewRevision as number }));
  const select = (r: RunResult) => act(() => productClient.creation.selectResult(projectId, item.id, "video", r.assetId));
  return <div className="storyboard-review-backdrop" role="presentation" onClick={onClose}>
    <section role="dialog" aria-modal="true" aria-label={`审核 ${item.name}`} className="storyboard-review" onClick={e => e.stopPropagation()}>
      <header><h2>审核 · {item.name}</h2><button type="button" onClick={onClose}>关闭</button></header>
      <p className="storyboard-review-note">审核通过、退回与选为镜头结果是独立操作；退回需填写说明。</p>
      <label className="storyboard-review-comment">审核说明<input value={comment} disabled={busy} onChange={e => setComment(e.target.value)} /></label>
      {error && <p role="alert" className="storyboard-error">{error}</p>}
      {!results ? <p role="status">正在读取候选…</p> : !results.length ? <p className="storyboard-empty-note">该运行没有可审核的结果。</p> :
        <ul className="storyboard-review-list">{results.map(r => {
          const asset = item.candidates.find(a => a.id === r.assetId);
          const chosen = r.selectedShotIds.includes(item.id) || item.shot?.selectedResultId === r.assetId;
          const pending = r.reviewState === "PENDING" && r.reviewRevision !== null && r.assetExists;
          return <li key={r.assetId} aria-label={`候选 ${r.name}`}>
            <div className="storyboard-cover">{asset ? <CandidateThumbnail asset={asset} playable /> : <span className="storyboard-cover-empty">{r.assetExists ? "无缩略图" : "素材不存在"}</span>}</div>
            <div><strong>{r.name}</strong><small>{r.reviewState ? reviewLabels[r.reviewState] ?? "审核状态未知" : "无审核记录"}{chosen ? " · 已选为结果" : ""}</small></div>
            <div className="storyboard-card-actions">
              {pending && <button type="button" disabled={busy} onClick={() => void review(r, "APPROVED")}>通过</button>}
              {pending && <button type="button" disabled={busy || !comment.trim()} title={comment.trim() ? undefined : "请先填写审核说明"} onClick={() => void review(r, "REJECTED")}>退回</button>}
              <button type="button" className="primary" disabled={busy || chosen || !r.assetExists || r.reviewState === "REJECTED"} onClick={() => void select(r)}>{chosen ? "当前结果" : "选为结果"}</button>
            </div>
          </li>;
        })}</ul>}
    </section>
  </div>;
}

function ShotRow({ item, selected, onSelect, onOpen, onReview }: { item: StoryboardShot; selected: boolean; onSelect: () => void; onOpen: () => void; onReview: () => void }) {
  return <div role="row" aria-label={item.name} aria-selected={selected} className={`storyboard-row${selected ? " selected" : ""}`} onClick={onSelect}>
    <span role="cell"><Cover item={item} /></span>
    <span role="cell" className="storyboard-row-title"><strong>{item.name}</strong><small>{item.shot?.prompt.trim() || "尚未填写提示词"}</small></span>
    <span role="cell"><StatusBadge item={item} /></span>
    <span role="cell">{shotDuration(item.shot)}</span>
    <span role="cell">{shotResolution(item.shot)}</span>
    <span role="cell">{item.candidates.length}</span>
    <span role="cell" className="storyboard-card-actions" onClick={e => e.stopPropagation()}>
      {canReview(item) && <button type="button" onClick={onReview}>审核</button>}
      <button type="button" onClick={onOpen}>{item.status === "TODO" || item.status === "FAILED" ? "生成" : "继续创作"}</button>
    </span>
  </div>;
}

function Cover({ item, playable = true }: { item: StoryboardShot; playable?: boolean }) {
  const asset = coverAsset(item); const percent = item.status === "GENERATING" ? runPercent(item.run) : null;
  return <div className="storyboard-cover">{asset ? <CandidateThumbnail asset={asset} playable={playable} /> : <span className="storyboard-cover-empty">暂无结果</span>}
    {percent !== null && <span className="storyboard-ring" style={{ ["--p" as string]: `${percent}` }} aria-label={`进度 ${percent}%`}>{percent}%</span>}
  </div>;
}
function StatusBadge({ item, compact }: { item: StoryboardShot; compact?: boolean }) {
  const percent = item.status === "GENERATING" ? runPercent(item.run) : null;
  return <span className={`storyboard-badge ${item.status.toLowerCase()}${compact ? " compact" : ""}`}>{item.error ? "读取失败" : storyboardStatusLabels[item.status]}{percent !== null && !compact ? ` ${percent}%` : ""}</span>;
}
function ShotCard({ item, selected, onSelect, onOpen, onRun, onReview }: { item: StoryboardShot; selected: boolean; onSelect: () => void; onOpen: () => void; onRun: () => void; onReview: () => void }) {
  const projectId = useContext(ThumbnailProjectContext);
  const prompt = item.shot?.prompt.trim();
  return <article className={`storyboard-card${selected ? " selected" : ""}`} aria-label={item.name} onClick={onSelect}>
    <Cover item={item} />
    <div className="storyboard-card-body">
      <header><strong>{item.name}</strong><StatusBadge item={item} /></header>
      <p className={prompt ? "" : "muted"}>{prompt || "尚未填写提示词"}</p>
      <ul className="storyboard-tags"><li>{shotDuration(item.shot)}</li><li>{shotResolution(item.shot)}</li><li>{item.candidates.length} 个候选</li></ul>
      {projectId && <ShotEntityChips projectId={projectId} shotId={item.id} />}
      {item.error && <small role="status">{item.error}</small>}
      <div className="storyboard-card-actions" onClick={e => e.stopPropagation()}>
        {canReview(item) && <button type="button" onClick={onReview}>审核</button>}
        <button type="button" onClick={onOpen}>查看</button>
        {item.run && <button type="button" onClick={onRun}>{item.status === "GENERATING" ? "查看运行" : "运行记录"}</button>}
        <button type="button" className={item.status === "GENERATING" ? "primary" : ""} onClick={onOpen}>{item.status === "TODO" || item.status === "FAILED" ? "生成" : "继续创作"}</button>
      </div>
    </div>
  </article>;
}
function ShotDetail({ item }: { item: StoryboardShot }) {
  const cover = coverAsset(item);
  const projectId = useContext(ThumbnailProjectContext);
  const refs = item.shot?.referenceAssetIds ?? [];
  return <div className="storyboard-detail-body">
    <div className="storyboard-detail-head"><Cover item={item} /><div><strong>{item.name}</strong><StatusBadge item={item} /></div></div>
    <dl>
      <div><dt>时长</dt><dd>{shotDuration(item.shot)}</dd></div>
      <div><dt>分辨率</dt><dd>{shotResolution(item.shot)}</dd></div>
      <div><dt>提示词摘要</dt><dd>{item.shot?.prompt.trim() || "尚未填写"}</dd></div>
      <div><dt>角色与场景</dt><dd>{projectId && <ShotEntityChips projectId={projectId} shotId={item.id} editable />}</dd></div>
      <div><dt>参考素材</dt><dd>{refs.length ? <div className="storyboard-ref-thumbs" aria-label="参考素材缩略图">{projectId && refs.map(id => <AssetImageThumb key={id} projectId={projectId} assetId={id} name={`参考 ${id.slice(0, 8)}`} size="lg" />)}<small>{refs.length} 张</small></div> : "0 张"}</dd></div>
      <div><dt>生成候选</dt><dd>{item.candidates.length} 个</dd></div>
      {item.run && <div><dt>最近运行</dt><dd>{item.run.title} · {item.run.progress.succeeded}/{item.run.progress.total}</dd></div>}
    </dl>
    <div className="storyboard-detail-result"><span>当前结果</span>{cover ? <CandidateThumbnail asset={cover} playable /> : <p className="storyboard-empty-note">尚无生成结果</p>}</div>
  </div>;
}
