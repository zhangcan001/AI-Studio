import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { useCallback, useEffect, useRef, useState } from "react";
import { useProjectSessionState } from "../../hooks/useProjectSessionState";
import { StoryEntityCategorySection } from "./StoryEntityCategorySection";
import type { StoryEntityKind } from "../../types/storyEntity";
import { ProjectMediaInspection } from "./MediaIntegrityPanel";
import { useLibraryController, type LibraryProps } from "./LibraryController";
import { LibraryDetailPanel } from "./LibraryDetail";
import { categories, resourceKey, sourceOriginLabel, subtypeLabel } from "./libraryModel";
import { LibraryThumbnail, LibraryThumbnailScope } from "./LibraryThumbnail";
import "./LibraryPage.css";

const ICON_PATHS: Record<string, string> = {
  all: "M12 6v6l4 2 M12 3a9 9 0 1 0 9 9a9 9 0 0 0-9-9z",
  media: "M3 4h8v7H3z M13 4h8v7h-8z M3 13h8v7H3z M13 13h8v7h-8z",
  images: "M4 5h16v14H4z M8.5 10a1.5 1.5 0 1 0 0-3a1.5 1.5 0 0 0 0 3z M20 15l-4.5-4.5L6 19",
  videos: "m9 7 9 5-9 5z M4 5h16v14H4z",
  audio: "M9 18V6l11-2v12 M6 18a3 3 0 1 0 3 3a3 3 0 0 0-3-3z M17 16a3 3 0 1 0 3 3a3 3 0 0 0-3-3z",
  prompts: "M4 20h4L18.5 9.5a2.1 2.1 0 0 0-3-3L5 17v3z",
  profiles: "M12 12a4 4 0 1 0-4-4a4 4 0 0 0 4 4z M5 20a7 7 0 0 1 14 0",
  "reference-sets": "M4 6h16 M4 12h16 M4 18h16",
  character: "M12 11a4 4 0 1 0-4-4a4 4 0 0 0 4 4z M6 20a6 6 0 0 1 12 0",
  scene: "M3 19h18 M5 19V9l7-5 7 5v10 M10 19v-5h4v5",
  prop: "M12 3l8 4.5v9L12 21l-8-4.5v-9z M12 12l8-4.5 M12 12v9 M12 12L4 7.5",
  composition: "M3 5h18v14H3z M9 5v14 M3 11h18",
};
function LibraryNavIcon({ name }: { name: string }) {
  const d = ICON_PATHS[name] ?? "M12 12h.01";
  return <span aria-hidden="true" className="library-nav-icon"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg></span>;
}

/** Library: toolbar + one-column category nav + responsive thumbnail grid + detail drawer column. */
export function LibraryPage(props:LibraryProps) {
  const draftFlags=useRef({entity:false,detail:false});
  const latestProps=useRef(props);latestProps.current=props;
  const safeNavigate=useCallback(async(next:Parameters<LibraryProps["navigate"]>[0])=>{
    const p=latestProps.current;
    // The mounted App owns global leave confirmation. Standalone callers use this fallback.
    if(!p.onDirtyChange&&(draftFlags.current.entity||draftFlags.current.detail)&&!window.confirm("当前资源编辑尚未保存，确定放弃并离开？"))return false;
    return p.navigate(next);
  },[]);
  const c=useLibraryController({...props,navigate:safeNavigate});
  const [searchDraft,setSearchDraft]=useState(c.search);
  const searchTimer=useRef<ReturnType<typeof setTimeout>|undefined>(undefined);
  useEffect(()=>{clearTimeout(searchTimer.current);setSearchDraft(c.search);},[props.route.projectId,c.search]);
  useEffect(()=>()=>clearTimeout(searchTimer.current),[]);
  const onSearchInput=(value:string)=>{
    setSearchDraft(value);
    clearTimeout(searchTimer.current);
    if(!value.trim()){c.setSearch(value);return;}
    searchTimer.current=setTimeout(()=>c.setSearch(value),150);
  };
  const advancedRef=useRef<HTMLDetailsElement>(null);
  useEffect(()=>{
    const closeOutside=(e:MouseEvent)=>{if(advancedRef.current?.open&&e.target instanceof Node&&!advancedRef.current.contains(e.target))advancedRef.current.open=false;};
    const closeEsc=(e:KeyboardEvent)=>{if(e.key==="Escape"&&advancedRef.current?.open)advancedRef.current.open=false;};
    document.addEventListener("mousedown",closeOutside);window.addEventListener("keydown",closeEsc);
    return()=>{document.removeEventListener("mousedown",closeOutside);window.removeEventListener("keydown",closeEsc);};
  },[]);
  const [batchMode,setBatchMode]=useState(false);
  const [selectedKeys,setSelectedKeys]=useState<string[]>([]);
  const [batchPlan,setBatchPlan]=useState<{allowed:import("../../product/libraryTypes").ResourceRef[];blocked:number}|undefined>();
  const [batchBusy,setBatchBusy]=useState(false);
  const [batchNotice,setBatchNotice]=useState<string>();
  useEffect(()=>{setBatchMode(false);setSelectedKeys([]);setBatchPlan(undefined);setBatchNotice(undefined);},[props.route.projectId,c.category]);
  const [importing,setImporting]=useState(false),[importNotice,setImportNotice]=useState<string>();
  const importLock=useRef(false),owner=useRef(props.route.projectId); owner.current=props.route.projectId;
  const importMedia=async(folder=false)=>{if(importLock.current)return;const project=props.route.projectId;importLock.current=true;setImporting(true);setImportNotice(undefined);try{const result=await productClient.creation.assetsImport(project,folder);if(owner.current!==project)return;if(!result.cancelled){setImportNotice(`导入成功 ${result.imported.length} 项，失败 ${result.failed.length} 项。${result.failed.map(f=>f.displayName+"："+f.error).join("；")}`);await c.refresh();}}catch(cause){if(owner.current===project)setImportNotice(normalizeProductError(cause).message);}finally{importLock.current=false;if(owner.current===project)setImporting(false);}};
  const entityDirty = useRef(false);
  const onEntityDirty = useCallback((dirty: boolean) => { entityDirty.current = dirty; draftFlags.current.entity=dirty;props.onDirtyChange?.(dirty||draftFlags.current.detail); }, [props.onDirtyChange]);
  const onDetailDirty=useCallback((dirty:boolean)=>{draftFlags.current.detail=dirty;props.onDirtyChange?.(dirty||draftFlags.current.entity);},[props.onDirtyChange]);
  useEffect(()=>()=>props.onDirtyChange?.(false),[props.onDirtyChange]);
  const confirmEntityLeave = () => !entityDirty.current || window.confirm("当前角色/场景/道具有未保存的编辑，确定放弃并切换？");
  // Character, scene, and prop libraries are separate views over project story entities.
  const [entityKind,setEntityKind]=useProjectSessionState<StoryEntityKind|null>(props.route.projectId, "library.entity-kind", null, (v): v is StoryEntityKind|null => v === null || (typeof v === "string" && ["character","scene","prop","composition"].includes(v)));
  const previousCategory=useRef({projectId:props.route.projectId,category:c.category});
  useEffect(()=>{const old=previousCategory.current;previousCategory.current={projectId:props.route.projectId,category:c.category};if(old.projectId===props.route.projectId&&old.category!==c.category)setEntityKind(null);},[props.route.projectId,c.category]);
  // An explicit deep link wins over remembered navigation, never hide its detail.
  useEffect(()=>{if(props.route.resource)setEntityKind(null);},[props.route.projectId,props.route.resource?.kind,props.route.resource?.id]);
  const visibleResources=c.list?.items.filter(item=>item.resourceRef.kind!=="profile");
  const inspectBatchDelete=async()=>{
    if(batchBusy||!selectedKeys.length||!visibleResources)return;
    const project=props.route.projectId;
    const refs=visibleResources.filter(item=>selectedKeys.includes(resourceKey(item.resourceRef))).map(item=>item.resourceRef);
    if(!refs.length)return;
    setBatchBusy(true);setBatchNotice(undefined);setBatchPlan(undefined);
    try{
      const checks=await Promise.all(refs.map(async ref=>({ref,res:await productClient.library.deletionInspect(project,ref)})));
      if(owner.current!==project)return;
      const allowed=checks.filter(x=>x.res.allowed).map(x=>x.ref);
      const blocked=checks.length-allowed.length;
      setBatchPlan({allowed,blocked});
      if(!allowed.length)setBatchNotice(`所选 ${checks.length} 项均有关联引用保护，无法直接删除。`);
    }catch(cause){if(owner.current===project)setBatchNotice(normalizeProductError(cause).message);}
    finally{if(owner.current===project)setBatchBusy(false);}
  };
  const executeBatchDelete=async()=>{
    if(batchBusy||!batchPlan?.allowed.length)return;
    const project=props.route.projectId,targets=batchPlan.allowed,skipped=batchPlan.blocked;
    setBatchBusy(true);
    try{
      for(const ref of targets){await productClient.library.delete(project,ref,true);}
      if(owner.current!==project)return;
      setSelectedKeys([]);setBatchPlan(undefined);
      setBatchNotice(`已批量删除 ${targets.length} 项${skipped?`（跳过受保护资源 ${skipped} 项）`:""}；历史任务和运行记录仍保留。`);
      await c.refresh();
    }catch(cause){if(owner.current===project)setBatchNotice(normalizeProductError(cause).message);}
    finally{if(owner.current===project)setBatchBusy(false);}
  };
  const showDetail=!entityKind&&Boolean(props.route.resource);
  const pager=c.list?.coverage==="keyset-page"&&(c.canPrevious||c.list.nextCursor)&&<nav className="library-pagination" aria-label="资源分页"><button type="button" disabled={!c.canPrevious||c.loading} onClick={c.previousPage}>上一页</button><span>第 {c.pageIndex+1} 页</span><button type="button" disabled={!c.list.nextCursor||c.loading} onClick={c.nextPage}>下一页</button></nav>;
  return <div className="library-page">
    <header className="library-toolbar">
      <h1>资源库</h1>
      {!entityKind&&<input type="search" className="library-search" aria-label="搜索资源名称" placeholder={c.category==="prompts"?"搜索提示词（名称 / 标签）":"搜索资源名称"} value={searchDraft} onChange={e=>onSearchInput(e.target.value)} onKeyDown={e=>{if(e.key==="Enter"){clearTimeout(searchTimer.current);c.setSearch(searchDraft);}}}/>}
      {c.mediaCategory&&!entityKind&&<div className="library-filters" aria-label="媒体筛选" title="卡片徽章标出外部导入 / 生成结果；来源与收藏筛选会在本会话内记住（按项目）。">
        <select aria-label="素材来源" value={c.sourceKind} onChange={e=>c.setSourceKind(e.target.value as typeof c.sourceKind)}><option value="all">全部来源</option><option value="source">外部导入</option><option value="generated">生成结果</option></select>
        <select aria-label="标签" value={c.tagId} onChange={e=>c.setTagId(e.target.value)}><option value="">全部标签</option>{c.tags.map(tag=><option key={tag.id} value={tag.id}>{tag.name}</option>)}</select>
        <label className="library-check"><input type="checkbox" checked={c.favoriteOnly} onChange={e=>c.setFavoriteOnly(e.target.checked)}/>仅收藏</label>
      </div>}
      {!entityKind&&<><button type="button" disabled={importing} onClick={()=>void importMedia()}>导入素材</button><button type="button" disabled={importing} onClick={()=>void importMedia(true)}>导入素材文件夹</button>{c.mediaCategory&&<button type="button" aria-pressed={batchMode} onClick={()=>{setBatchMode(v=>!v);setSelectedKeys([]);setBatchPlan(undefined);}}>批量管理</button>}<button type="button" onClick={()=>void c.refresh()}>刷新</button></>}
      {importNotice&&<p role="status">{importNotice}</p>}
      {batchNotice&&<p role="status">{batchNotice}</p>}
      <details ref={advancedRef} className="library-advanced"><summary>高级</summary><div className="library-advanced-body">
        <button type="button" onClick={()=>{if(advancedRef.current)advancedRef.current.open=false;void safeNavigate({...props.route,filter:"advanced-assets"});}}>素材与参考集管理</button>
        <button type="button" onClick={()=>{if(advancedRef.current)advancedRef.current.open=false;void safeNavigate({...props.route,filter:"advanced-prompts"});}}>高级提示词编辑器</button>
        {c.mediaCategory&&<ProjectMediaInspection inspection={c.mediaInspection} open={id=>{if(advancedRef.current)advancedRef.current.open=false;c.open({kind:"asset",id});}}/>}
      </div></details>
    </header>
    {batchMode&&!entityKind&&<div className="library-batch-bar" role="region" aria-label="批量资源操作">
      <span>已选择 {selectedKeys.length} 项</span>
      <button type="button" disabled={!visibleResources?.length||batchBusy} onClick={()=>{setBatchPlan(undefined);setSelectedKeys(visibleResources?.map(i=>resourceKey(i.resourceRef))??[]);}}>全选本页</button>
      <button type="button" disabled={!selectedKeys.length||batchBusy} onClick={()=>{setBatchPlan(undefined);setSelectedKeys([]);}}>清空选择</button>
      <button type="button" disabled={!selectedKeys.length||batchBusy} onClick={()=>void inspectBatchDelete()}>{batchBusy?"正在检查引用…":"检查并批量删除"}</button>
      {batchPlan&&batchPlan.allowed.length>0&&<button type="button" className="primary" disabled={batchBusy} onClick={()=>void executeBatchDelete()}>确认删除可删项（{batchPlan.allowed.length} 项{batchPlan.blocked?`，跳过受保护 ${batchPlan.blocked} 项`:""}）</button>}
      <button type="button" disabled={batchBusy} onClick={()=>{setBatchMode(false);setSelectedKeys([]);setBatchPlan(undefined);}}>完成</button>
    </div>}
    {c.category==="profiles"&&<p role="note">旧设定入口已停用，请使用角色、场景、道具或构图分类。旧记录仍保留用于历史查看与备份。</p>}
    {c.category==="prompts"&&<p role="note">提示词分类入口已停用。当前无法在资源库中新增提示词，已有记录仍保留用于历史查看与备份。</p>}
    {c.queryError&&<p role="alert">{c.queryError}</p>}{c.actionError&&<p role="alert">{c.actionError}</p>}{c.notice&&<p role="status">{c.notice}</p>}{c.loading&&<p role="status">正在读取资源…</p>}
    <div className="library-layout" data-detail={showDetail?"open":"closed"}>
      <nav aria-label="资源分类" className="library-nav">
        {categories.filter(cat=>cat.key!=="profiles"&&cat.key!=="prompts").map(cat=><button type="button" key={cat.key} aria-pressed={!entityKind&&cat.key===c.category} onClick={()=>{void Promise.resolve(safeNavigate({kind:"library",projectId:props.route.projectId,filter:cat.key})).then(ok=>{if(ok!==false)setEntityKind(null);});}}><LibraryNavIcon name={cat.key}/>{cat.label}</button>)}
        {([{kind:"character",label:"角色"},{kind:"scene",label:"场景"},{kind:"prop",label:"道具"},{kind:"composition",label:"构图"}] as const).map(item=><button type="button" key={item.kind} aria-pressed={entityKind===item.kind} onClick={()=>{if(entityKind===item.kind)return;if(!draftFlags.current.detail&&confirmEntityLeave())setEntityKind(item.kind);else if(draftFlags.current.detail&&window.confirm("当前资源编辑尚未保存，确定放弃并切换？"))setEntityKind(item.kind);}}><LibraryNavIcon name={item.kind}/>{item.label}</button>)}
      </nav>
      {entityKind ? <StoryEntityCategorySection key={`${props.route.projectId}:${entityKind}`} projectId={props.route.projectId} kind={entityKind} onDirtyChange={onEntityDirty} /> : <LibraryThumbnailScope><section className="library-list" aria-label="资源列表">
        <header className="library-list-head"><h2>{categories.find(cat=>cat.key===c.category)?.label}</h2>{c.category==="all"&&<small>各类近期摘要；切换到媒体 / 图片 / 视频 / 音频后可按来源、收藏或标签查找。</small>}</header>
        {c.loading&&!c.list&&<div className="library-skeleton-grid" aria-hidden="true"><span className="library-skeleton-card"/><span className="library-skeleton-card"/><span className="library-skeleton-card"/><span className="library-skeleton-card"/></div>}
        <div className="library-grid">{visibleResources?.map(item=>{const key=resourceKey(item.resourceRef);const checked=batchMode&&selectedKeys.includes(key);const assetMedia=item.resourceRef.kind==="asset"&&(item.subtype==="image"||item.subtype==="video"||item.subtype==="audio")?item.subtype:undefined;return <button type="button" className="library-card" data-selected={checked||undefined} draggable={Boolean(assetMedia)} onDragStart={e=>{if(assetMedia&&item.resourceRef.kind==="asset"){e.dataTransfer.setData("application/x-aistudio-asset",JSON.stringify({projectId:props.route.projectId,assetId:item.resourceRef.id,mediaKind:assetMedia}));e.dataTransfer.effectAllowed="copy";}}} key={`${c.category}:${c.search.trim()}:${c.sourceKind}:${c.favoriteOnly}:${c.tagId}:${props.route.projectId}:${c.pageIndex}:${key}`} aria-pressed={batchMode?checked:resourceKey(props.route.resource)===key} onClick={()=>{if(batchMode){setBatchPlan(undefined);setSelectedKeys(prev=>prev.includes(key)?prev.filter(k=>k!==key):[...prev,key]);}else c.open(item.resourceRef);}}>{batchMode&&<span className="library-card-check" aria-hidden="true">{checked?"✓ 已选":"选择"}</span>}<LibraryThumbnail projectId={props.route.projectId} item={item}/><strong title={item.title}>{item.title}</strong><span className="library-card-meta">{item.resourceRef.kind==="reference-set"?"参考集":subtypeLabel(item.subtype)}{sourceOriginLabel(item.sourceOrigin)?<span className={`library-source-badge library-source-badge-${item.sourceOrigin}`}>{sourceOriginLabel(item.sourceOrigin)}</span>:null}</span><time dateTime={item.createdAt}>{new Date(item.createdAt).toLocaleString("zh-CN")}</time></button>;})}</div>
        {c.list&&!visibleResources?.length&&<p className="library-empty">{c.mediaCategory&&c.sourceKind==="source"?"当前筛选下没有外部导入素材。请先导入图片/视频，或切换来源筛选。":c.mediaCategory&&c.sourceKind==="generated"?"当前筛选下没有生成结果。请切换来源或清除筛选。":"当前分类 / 搜索没有资源。"}</p>}
        <footer className="library-list-foot">{pager}<small>{c.list?.coverageMessage}</small></footer>
      </section></LibraryThumbnailScope>}
      {showDetail&&<LibraryDetailPanel {...props} navigate={safeNavigate} onDirtyChange={onDetailDirty} controller={c}/>}
    </div>
  </div>;
}
