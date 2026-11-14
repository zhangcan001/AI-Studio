import { ShotReviewDialog } from "./ShotReviewDialog";
import { ShotEntityChips } from "./ShotEntityChips";
import { StoryboardShotRows } from "./StoryboardShotRows";
import { useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { subscribeRunInvalidation } from "../../product/runInvalidation";
import type { AppRoute } from "../../app/routes/types";
import { AssetImageThumb, CandidateThumbnail, MediaLightboxScope, ThumbnailProjectContext } from "./CreateMediaPreview";
import { candidateReviewSummary, coverAsset, deriveStoryboardStatus, runProgressText, runPercent, shotDuration, shotResolution, storyboardStats, storyboardStatusLabels, type StoryboardShot } from "./storyboardModel";
import "./CreatePage.css";
import { CreatePage } from "./CreatePage";
import type { CreateProps } from "./CreateController";
import { useStudioStore } from "../../stores/studioStore";
import { useProjectSessionState } from "../../hooks/useProjectSessionState";
import type { CreationShotSummary } from "../../product/types";

/** Bounded per-shot reads keep the workbench responsive on large projects. */
export const STORYBOARD_MAX_DETAIL_SHOTS = 60;
const POLL_MS = 8000;
const EVENT_REFRESH_MS = 500;

interface Props extends CreateProps {
  renderBatch?: (props:{projectId:string;onClose:()=>void;onQueue:(id?:string)=>void;onShot:(id:string)=>void})=>ReactNode;
  renderFolderImport?: (props:{projectId:string;onImported:()=>unknown;onCancel:()=>void;onOpenQueue:(batchId:string)=>void;onOpenShot:(shotId:string)=>unknown;onOpenLegacy:()=>void})=>ReactNode;
  // Cross-feature composition belongs to the app, not the Create feature.
  renderRuns?: (route: Extract<AppRoute, { kind: "runs" }>, navigate: CreateProps["navigate"]) => ReactNode;
}

/**
 * Storyboard workbench: the primary daily creation surface. It only reads the
 * product facade (creation context + run status). The embedded Create controller
 * retains Studio Store ownership and submits only through the Production Queue.
 */
export function StoryboardWorkbench({ route, navigate, runtime, onDirtyChange, renderRuns, renderFolderImport, renderBatch }: Props) {
  const [items, setItems] = useState<StoryboardShot[]>();
  const [index, setIndex] = useState<{projectId: string; shots: CreationShotSummary[]}>();
  const [truncated, setTruncated] = useState(0);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState(false);
  const createInFlight = useRef(false);
  const [view, setViewState] = useState<StoryboardView>(() => readView());
  const setView = (next: StoryboardView) => { setViewState(next); try { window.localStorage.setItem(VIEW_KEY, next); } catch { /* storage unavailable */ } };
  const [folderImportOpen,setFolderImportOpen]=useState(false);
  useEffect(()=>setFolderImportOpen(false),[route.projectId]);
  const [orderNotice,setOrderNotice]=useState<string>();
  const [focusId,setFocusId]=useState<string>();
  const pendingScroll=useRef<string | undefined>(undefined);
  const [orderError,setOrderError] = useState<string>();
  const [exporting,setExporting] = useState(false);
  const exportLock=useRef(false);
  const [delivery,setDelivery] = useState<import("../../product/types").SelectedVideoExport>();
  const [openExport,setOpenExport]=useState(false);
  const [copyNotice,setCopyNotice]=useState<string>();
  const [batchOpen, setBatchOpen] = useState(false);
  const [page, setPage] = useState(0);
  const orderLock = useRef(false);
  const [reviewId, setReviewId] = useState<string>();
  const [selectedId, setSelectedId] = useState<string>();
  const [runRoute, setRunRoute] = useState<Extract<AppRoute, { kind: "runs" }>>();
  const [shotQuery, setShotQuery] = useProjectSessionState(route.projectId, "storyboard.inline-search", "", (v): v is string => typeof v === "string");
  const libraryIntent = useStudioStore(state => state.pendingLibraryIntent);
  const runIntent = useStudioStore(state => state.pendingRunIntent);
  const matchingRunIntent = runIntent?.projectId === route.projectId && runIntent.stage === route.stage;
  const editing = Boolean(route.shotId || libraryIntent || matchingRunIntent);
  const epoch = useRef(0);
  const inFlight = useRef(false);
  const pendingRefresh = useRef(false);
  const latestLoad = useRef<(background?: boolean) => Promise<void>>(async () => {});
  const [queueActive, setQueueActive] = useState(false);
  const mounted = useRef(true);
  const projectId = route.projectId;
  const scope = useRef(projectId);
  scope.current = projectId;
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);

  const load = useCallback(async (background = false) => {
    if (inFlight.current) { if (!background) pendingRefresh.current = true; return; }
    inFlight.current = true; const token = ++epoch.current;
    // Progress reads must not toggle every action button. Foreground reads and
    // the existing row/create mutation locks still prevent duplicate actions.
    if (!background) setBusy(true);
    try {
      // Formal Queue/Run read model: a waiting item may have no Task yet, so
      // shot.recentRun alone cannot tell us whether a refresh is still needed.
      const activeRuns = Promise.resolve().then(() => productClient.run.list(projectId, "active", null)).catch(() => null);
      const workbench = await productClient.creation.workbench(projectId, "video", STORYBOARD_MAX_DETAIL_SHOTS, ...(page ? [page * STORYBOARD_MAX_DETAIL_SHOTS] as [number] : []));
      const runKeys = new Map<string, NonNullable<(typeof workbench.items)[number]["shot"]["recentRun"]>>();
      for (const item of workbench.items) {
        const ref = item.shot.recentRun;
        if (ref) runKeys.set(`${ref.source}:${ref.id}`, ref);
      }
      const runs = new Map<string, Awaited<ReturnType<typeof productClient.run.get>>>();
      await Promise.all([...runKeys.entries()].map(async ([key, ref]) => {
        try { runs.set(key, await productClient.run.get(projectId, ref)); } catch { /* keep null */ }
      }));
      const active = await activeRuns;
      // Detail inputs resolve formal no-Task Queue items to their owned Shot.
      const activeDetails = await Promise.all((active?.items ?? []).filter(r => r.projectId === projectId && ["QUEUED", "RUNNING", "PAUSED"].includes(r.status)).map(async r => { try { return await productClient.run.get(projectId, r.ref); } catch { return null; } }));
      const activeByShot = new Map<string, NonNullable<typeof activeDetails[number]>>();
      for (const run of activeDetails) { if (!run || run.projectId !== projectId) continue; for (const input of run.detail?.inputs ?? []) { if (input.shotId && !input.taskId && input.executionStatus === "PENDING" && !activeByShot.has(input.shotId)) activeByShot.set(input.shotId, { ...run, status: run.status === "PAUSED" ? "PAUSED" : "QUEUED" }); } }
      const result: StoryboardShot[] = workbench.items.map(({ shot, candidates }) => {
        const video = candidates.filter(asset => asset.mediaKind === "video");
        const run = activeByShot.get(shot.summary.id) ?? (shot.recentRun ? runs.get(`${shot.recentRun.source}:${shot.recentRun.id}`) ?? null : null);
        return { id: shot.summary.id, name: shot.summary.name, ordinal: shot.summary.ordinal, shot, candidates: video, run, status: deriveStoryboardStatus(shot, video, run) };
      });
      if (token !== epoch.current) return;
      setQueueActive(Boolean(active?.items?.length));
      setIndex({projectId, shots: workbench.shots});
      setItems(result); setTruncated(workbench.truncated); setError(undefined);
    } catch (cause) { if (token === epoch.current) setError(normalizeProductError(cause).message); }
    finally { if (token === epoch.current) { inFlight.current = false; setBusy(false); if (pendingRefresh.current && mounted.current) { pendingRefresh.current = false; void latestLoad.current(true); } } }
  }, [projectId, page]);

  latestLoad.current = load;
  useEffect(() => { pendingRefresh.current = false; void load(); return () => { epoch.current++; inFlight.current = false; pendingRefresh.current = false; }; }, [load]);
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
  const shotIndex = index?.projectId === projectId ? index.shots : [];
  useEffect(() => {
    if (!route.shotId || index?.projectId !== projectId) return;
    const position = index.shots.findIndex(shot => shot.id === route.shotId);
    if (position >= 0) setPage(Math.floor(position / STORYBOARD_MAX_DETAIL_SHOTS));
  }, [projectId, route.shotId, index]);
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
  const exportSelected = async () => {
    if(exportLock.current)return;exportLock.current=true;setExporting(true);
    try{const result=await productClient.creation.exportSelected(projectId,openExport);if(mounted.current&&scope.current===projectId&&result)setDelivery(result);}
    catch(cause){if(mounted.current&&scope.current===projectId)setError(normalizeProductError(cause).message);}
    finally{exportLock.current=false;if(mounted.current&&scope.current===projectId)setExporting(false);}
  };
  useEffect(()=>{
    const id=pendingScroll.current;if(!id||!items?.some(i=>i.id===id))return;
    const element=document.getElementById(`storyboard-shot-${id}`);if(element){element.scrollIntoView?.({block:"nearest"});pendingScroll.current=undefined;}
  },[items]);
  const locateOrderedShot=(id:string,ids:string[],message:string)=>{
    const position=ids.indexOf(id);if(position<0)return;
    const nextPage=Math.floor(position/STORYBOARD_MAX_DETAIL_SHOTS);
    pendingScroll.current=id;setFocusId(id);setSelectedId(id);setOrderNotice(`${message}：第 ${position+1} 镜，第 ${nextPage+1} 页。若被筛选隐藏，请清除筛选查看。`);
    if(nextPage!==page)setPage(nextPage);else void load();
  };
  const moveShot = async (id: string, delta: -1 | 1) => {
    if (orderLock.current) return;
    orderLock.current = true; setOrderError(undefined);setOrderNotice(undefined);
    try {
      const shots = await productClient.creation.shotsList(projectId);
      const ids = shots.filter(s => s.projectId === projectId).sort((a,b) => a.ordinal-b.ordinal || a.id.localeCompare(b.id)).map(s => s.id);
      const i = ids.indexOf(id), j = i + delta;
      if (i < 0 || j < 0 || j >= ids.length) return;
      [ids[i],ids[j]] = [ids[j],ids[i]];
      await productClient.creation.shotsReorder(projectId,ids);
      if (mounted.current && scope.current === projectId) locateOrderedShot(id,ids,"镜头已移动");
    } catch(cause) { if (mounted.current && scope.current === projectId) setOrderError(normalizeProductError(cause).message); }
    finally { orderLock.current=false; }
  };
  const insertShot = async (afterId: string) => {
    if (orderLock.current) return;
    orderLock.current=true; setOrderError(undefined);
    let created: string | undefined;let positioned=false;
    try {
      const shot=await productClient.creation.createShot(projectId); created=shot.id;
      const shots=await productClient.creation.shotsList(projectId);
      const ids=shots.filter(s=>s.projectId===projectId).sort((a,b)=>a.ordinal-b.ordinal || a.id.localeCompare(b.id)).map(s=>s.id).filter(id=>id!==created);
      const index=ids.indexOf(afterId); if(index<0) throw new Error("插入位置已变化；新镜头保留在末尾，请刷新后移动。");
      ids.splice(index+1,0,shot.id); await productClient.creation.shotsReorder(projectId,ids);
      if(mounted.current&&scope.current===projectId){positioned=true;locateOrderedShot(shot.id,ids,"新镜头已插入");}
    } catch(cause) { if(mounted.current&&scope.current===projectId)setOrderError(`${created ? "新镜头已创建；排序未完成，请刷新后移动。" : ""}${normalizeProductError(cause).message}`); }
    finally { orderLock.current=false; if(!positioned&&mounted.current&&scope.current===projectId)void load(); }
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
        <div><dt>镜头总数</dt><dd>{shotIndex.length}</dd></div>
        <div data-tone="success"><dt>已选用</dt><dd>{stats.completed}</dd></div>
        <div data-tone="info"><dt>生成中</dt><dd>{stats.generating}</dd></div>
        <div data-tone="muted"><dt>待制作</dt><dd>{stats.todo}</dd></div>
      </dl>
      {truncated > 0 && <small role="note">状态统计仅覆盖已加载的 {stats.total} 个镜头。</small>}
      <div className="storyboard-header-actions">
        <button type="button" onClick={() => setRunRoute({ kind: "runs", projectId })}>任务队列与运行记录</button>
        <button type="button" onClick={() => setFolderImportOpen(true)} disabled={!renderFolderImport}>导入分镜脚本</button>
        <button type="button" className="primary" disabled={!renderBatch} onClick={() => setBatchOpen(true)}>批量准备视频</button><button type="button" disabled={exporting} onClick={()=>void exportSelected()}>{exporting?"正在导出…":"导出全项目已选用视频"}</button><label><input type="checkbox" checked={openExport} disabled={exporting} onChange={e=>setOpenExport(e.target.checked)}/>导出后打开目录</label>
      </div>
    </header>
    <p className="muted" role="note">导出范围为整个项目的已选用视频，不受当前页或筛选影响；选用不等于审核通过。</p>
    {delivery && <section aria-label="导出回执"><p role="status">已导出 {delivery.exported} 项，失败 {delivery.failed} 项。目录：{delivery.directory}</p><button type="button" onClick={()=>{void Promise.resolve().then(()=>navigator.clipboard.writeText(delivery.directory)).then(()=>setCopyNotice("导出路径已复制。"),()=>setCopyNotice("复制失败，可手动选择目录文本复制。"));}}>复制导出路径</button>{copyNotice&&<p role="status">{copyNotice}</p>}{delivery.manifestError&&<p role="alert">{delivery.manifestError}</p>}{delivery.openError&&<p role="alert">{delivery.openError}</p>}<ul>{delivery.entries?.filter(e=>e.error).map(e=><li key={e.shotId}>第 {e.ordinal+1} 镜 · {e.shotName}：{e.error}</li>)}</ul><small>export-manifest.json 保留镜头、素材和文件名对应关系，不包含绝对源路径。</small></section>}
    {orderError && <p role="alert">{orderError}</p>}{orderNotice&&<p role="status">{orderNotice}</p>}
    {error && <p role="alert" className="storyboard-error">{error}</p>}
    {editing ? <div className="storyboard-editing-body">
      <aside className="storyboard-panel storyboard-shot-nav" aria-label="镜头切换">
        <header><h2>镜头列表</h2><button type="button" onClick={() => void navigate({ kind: "create", projectId, stage: "video" })}>展开分镜列表</button></header>
        <input type="search" aria-label="搜索操作镜头" placeholder="搜索镜头" value={shotQuery} onChange={e => setShotQuery(e.target.value)} />
        {!items && <p role="status">正在读取分镜…</p>}
        <ol>{shotIndex.filter(summary => !shotQuery.trim() || `${summary.name} ${items?.find(i => i.id === summary.id)?.shot?.prompt ?? ""}`.toLowerCase().includes(shotQuery.trim().toLowerCase())).map(summary => { const item = items?.find(i => i.id === summary.id); return <li key={summary.id}>
          <button type="button" aria-label={`操作${summary.name}`} aria-current={route.shotId === summary.id ? "true" : undefined} onClick={() => openShot(summary.id)}>
            {item && <Cover item={item} playable={false} />}<span><strong>{summary.name}</strong>{item && <><small>{shotDuration(item.shot)} · {shotResolution(item.shot)}</small><StatusBadge item={item} compact /></>}</span>
          </button>
        </li>; })}</ol>
        {truncated > 0 && <small>可按全项目镜头名搜索；提示词搜索与状态仅覆盖已加载详情的镜头。</small>}
      </aside>
      <section className="storyboard-inline-editor" aria-label="镜头操作">
        <header><span>提示词 · 参数 · 素材 · 生成 · 候选</span>{route.shotId && selected?.id === route.shotId && canReview(selected) && <button type="button" onClick={() => setReviewId(selected.id)}>人工审核</button>}</header>
        {matchingRunIntent && !route.shotId && <p role="note">请选择目标镜头；历史输入只载入该镜头的未保存草稿，不会自动保存或生成。</p>}
        <section className="storyboard-inline-create" aria-label="镜头内联创作"><CreatePage route={route} runtime={runtime} navigate={navigateInline} onDirtyChange={onDirtyChange} embedded onContextChange={load} /></section>
      </section>
    </div> : !items ? <p role="status" className="storyboard-loading">正在读取分镜…</p> : <div className="storyboard-body">
      <aside className="storyboard-panel storyboard-tree" aria-label="剧集与场景">
        <header><h2>剧集与场景</h2><button type="button" disabled={busy || creating} onClick={() => void createShot()}>+ 新建镜头</button></header>
        <div className="storyboard-tree-group"><span>全部镜头</span><span className="storyboard-count">{shotIndex.length}</span></div>
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
          : view === "shots" ? <StoryboardShotRows focusId={focusId} projectId={projectId} items={items} shotIndex={shotIndex} busy={busy || creating} onOpen={openShot} onCreate={() => void createShot(false)} onChanged={() => void load()} onReview={id => setReviewId(id)} onMove={(id,d)=>void moveShot(id,d)} onInsert={id=>void insertShot(id)} onQueue={ref => setRunRoute({ kind: "runs", projectId, run: ref })} />
          : view === "grid" ? <div className="storyboard-cards grid" data-view="grid">{items.map(item => <ShotCard key={item.id} item={item} selected={selected?.id === item.id} onSelect={() => setSelectedId(item.id)} onOpen={() => openShot(item.id)} onRun={() => openRun(item)} onReview={() => setReviewId(item.id)} />)}</div>
          : <div className="storyboard-rows" role="table" aria-label="镜头列表（列表模式）" data-view="list">
            <div role="row" className="storyboard-row head"><span role="columnheader">缩略图</span><span role="columnheader">镜头</span><span role="columnheader">状态</span><span role="columnheader">时长</span><span role="columnheader">分辨率</span><span role="columnheader">候选</span><span role="columnheader">操作</span></div>
            {items.map(item => <ShotRow key={item.id} item={item} selected={selected?.id === item.id} onSelect={() => setSelectedId(item.id)} onOpen={() => openShot(item.id)} onReview={() => setReviewId(item.id)} />)}
          </div>}
        <nav className="storyboard-pagination" aria-label="分镜详情分页"><button type="button" disabled={busy || page===0} onClick={()=>setPage(p=>p-1)}>上一页镜头</button><span>第 {page+1} 页 · 当前页 {items.length} 个 / 全项目 {shotIndex.length} 个；提示词与状态筛选作用于当前页</span><button type="button" disabled={busy || truncated<=0} onClick={()=>setPage(p=>p+1)}>下一页镜头</button></nav>
        {truncated > 0 && <small className="storyboard-truncated">另有 {truncated} 个镜头未在此展开，可通过“下一页镜头”查看。</small>}
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
    {reviewItem && <ShotReviewDialog key={`${projectId}:${reviewItem.id}`} projectId={projectId} item={reviewItem} onClose={() => setReviewId(undefined)} onChanged={() => void load()} onEdit={() => { void Promise.resolve(openShot(reviewItem.id)).then(ok => { if (ok !== false && mounted.current && scope.current === projectId) setReviewId(undefined); }); }} onNext={async()=>{
      const current=reviewItem.id, owner=projectId;
      const position=(items??[]).findIndex(i=>i.id===current);
      for(const candidate of (items??[]).slice(position+1)){
        if(!canReview(candidate)||!candidate.run)continue;
        const result=await productClient.run.resultsGet(owner,candidate.run.ref);
        if(!mounted.current||scope.current!==owner)return;
        if(result.some(r=>r.reviewState==="PENDING"&&r.assetExists&&candidate.candidates.some(a=>a.id===r.assetId))){setReviewId(candidate.id);return;}
      }
      return truncated>0?"本页后面没有待审核候选；可关闭窗口，翻到下一页继续。":"已到本页末尾，后面没有待审核候选。不会循环跳回，也不会自动选用或审核。";
    }} />}
    {batchOpen && renderBatch?.({projectId,onClose:()=>setBatchOpen(false),onQueue:id=>{setBatchOpen(false);setRunRoute({kind:"runs",projectId,run:id?{source:"queue-batch",id}:undefined});},onShot:id=>{void Promise.resolve(openShot(id)).then(ok=>{if(ok!==false&&mounted.current&&scope.current===projectId)setBatchOpen(false);});}})}
    {folderImportOpen && renderFolderImport?.({projectId,onImported:()=>load(),onCancel:()=>setFolderImportOpen(false),onOpenShot:async(id)=>{const owner=projectId;try{const accepted=await openShot(id);if(accepted!==false&&mounted.current&&scope.current===owner)setFolderImportOpen(false);}catch(cause){if(mounted.current&&scope.current===owner)setError(`无法打开镜头：${normalizeProductError(cause).message}`);}},onOpenQueue:(id)=>{setFolderImportOpen(false);setRunRoute({kind:"runs",projectId,run:{source:"queue-batch",id}});},onOpenLegacy:()=>{setFolderImportOpen(false);void navigate({kind:"project-settings",projectId,section:"advanced-production"});}})}
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
    {percent !== null && <span className="storyboard-ring" style={{ ["--p" as string]: `${percent}` }} title="任务完成比例，不是视频推理百分比" aria-label={runProgressText(item.run)}>{item.run ? `${item.run.progress.succeeded+item.run.progress.failed+item.run.progress.cancelled}/${item.run.progress.total}` : "—"}</span>}
  </div>;
}
function StatusBadge({ item, compact }: { item: StoryboardShot; compact?: boolean }) {
  const percent = item.status === "GENERATING" ? runPercent(item.run) : null;
  return <span className={`storyboard-badge ${item.status.toLowerCase()}${compact ? " compact" : ""}`}>{item.error ? "读取失败" : storyboardStatusLabels[item.status]}{percent !== null && !compact ? ` · ${runProgressText(item.run)}` : ""}</span>;
}
function ShotCard({ item, selected, onSelect, onOpen, onRun, onReview }: { item: StoryboardShot; selected: boolean; onSelect: () => void; onOpen: () => void; onRun: () => void; onReview: () => void }) {
  const projectId = useContext(ThumbnailProjectContext);
  const prompt = item.shot?.prompt.trim();
  return <article className={`storyboard-card${selected ? " selected" : ""}`} aria-label={item.name} onClick={onSelect}>
    <Cover item={item} />
    <div className="storyboard-card-body">
      <header><strong>{item.name}</strong><StatusBadge item={item} /></header><small>{candidateReviewSummary(item.candidates)}</small>
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
