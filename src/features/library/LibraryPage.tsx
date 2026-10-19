import { useEffect, useState } from "react";
import { StoryEntityCategorySection } from "./StoryEntityCategorySection";
import type { StoryEntityKind } from "../../types/storyEntity";
import { ProjectMediaInspection } from "./MediaIntegrityPanel";
import { useLibraryController, type LibraryProps } from "./LibraryController";
import { LibraryDetailPanel } from "./LibraryDetail";
import { categories, resourceKey, sourceOriginLabel, subtypeLabel } from "./libraryModel";
import { LibraryThumbnail, LibraryThumbnailScope } from "./LibraryThumbnail";
import "./LibraryPage.css";

const ICONS: Record<string, string> = { all: "◷", media: "▦", images: "▣", videos: "▶", audio: "♪", prompts: "✎", profiles: "☺", "reference-sets": "▤", character: "♙", scene: "⌂", prop: "◆" };

/** Library: toolbar + one-column category nav + responsive thumbnail grid + detail drawer column. */
export function LibraryPage(props:LibraryProps) {
  const c=useLibraryController(props);
  // Character, scene, and prop libraries are separate views over project story entities.
  const [entityKind,setEntityKind]=useState<StoryEntityKind|null>(null);
  useEffect(()=>{setEntityKind(null);},[props.route.projectId,c.category]);
  const showDetail=!entityKind&&Boolean(props.route.resource);
  const pager=c.list?.coverage==="keyset-page"&&(c.canPrevious||c.list.nextCursor)&&<nav className="library-pagination" aria-label="资源分页"><button type="button" disabled={!c.canPrevious||c.loading} onClick={c.previousPage}>上一页</button><span>第 {c.pageIndex+1} 页</span><button type="button" disabled={!c.list.nextCursor||c.loading} onClick={c.nextPage}>下一页</button></nav>;
  return <div className="library-page">
    <header className="library-toolbar">
      <h1>资源库</h1>
      {!entityKind&&<input type="search" className="library-search" aria-label="搜索资源名称" placeholder={c.category==="prompts"?"搜索提示词（名称 / 标签）":"搜索资源名称"} value={c.search} onChange={e=>c.setSearch(e.target.value)}/>}
      {c.mediaCategory&&!entityKind&&<div className="library-filters" aria-label="媒体筛选" title="卡片徽章标出外部导入 / 生成结果；来源与收藏筛选会在本会话内记住（按项目）。">
        <select aria-label="素材来源" value={c.sourceKind} onChange={e=>c.setSourceKind(e.target.value as typeof c.sourceKind)}><option value="all">全部来源</option><option value="source">外部导入</option><option value="generated">生成结果</option></select>
        <select aria-label="标签" value={c.tagId} onChange={e=>c.setTagId(e.target.value)}><option value="">全部标签</option>{c.tags.map(tag=><option key={tag.id} value={tag.id}>{tag.name}</option>)}</select>
        <label className="library-check"><input type="checkbox" checked={c.favoriteOnly} onChange={e=>c.setFavoriteOnly(e.target.checked)}/>仅收藏</label>
      </div>}
      {!entityKind&&<button type="button" onClick={()=>void c.refresh()}>刷新</button>}
      <details className="library-advanced"><summary>高级</summary><div className="library-advanced-body">
        <button type="button" onClick={()=>props.navigate({...props.route,filter:"advanced-assets"})}>素材导入 / 高级设定</button>
        <button type="button" onClick={()=>props.navigate({...props.route,filter:"advanced-prompts"})}>高级提示词编辑器</button>
        {c.mediaCategory&&<ProjectMediaInspection inspection={c.mediaInspection} open={id=>c.open({kind:"asset",id})}/>}
      </div></details>
    </header>
    {c.queryError&&<p role="alert">{c.queryError}</p>}{c.actionError&&<p role="alert">{c.actionError}</p>}{c.notice&&<p role="status">{c.notice}</p>}{c.loading&&<p role="status">正在读取资源…</p>}
    <div className="library-layout" data-detail={showDetail?"open":"closed"}>
      <nav aria-label="资源分类" className="library-nav">
        {categories.filter(cat=>cat.key!=="profiles").map(cat=><button type="button" key={cat.key} aria-pressed={!entityKind&&cat.key===c.category} onClick={()=>{setEntityKind(null);props.navigate({kind:"library",projectId:props.route.projectId,filter:cat.key});}}><span aria-hidden="true" className="library-nav-icon">{ICONS[cat.key]??"•"}</span>{cat.label}</button>)}
        {([{kind:"character",label:"角色"},{kind:"scene",label:"场景"},{kind:"prop",label:"道具"}] as const).map(item=><button type="button" key={item.kind} aria-pressed={entityKind===item.kind} onClick={()=>setEntityKind(item.kind)}><span aria-hidden="true" className="library-nav-icon">{ICONS[item.kind]}</span>{item.label}</button>)}
        <button type="button" aria-pressed={!entityKind&&c.category==="profiles"} onClick={()=>{setEntityKind(null);props.navigate({kind:"library",projectId:props.route.projectId,filter:"profiles"});}}><span aria-hidden="true" className="library-nav-icon">✦</span>旧设定 · 风格</button>
      </nav>
      {entityKind ? <StoryEntityCategorySection key={`${props.route.projectId}:${entityKind}`} projectId={props.route.projectId} kind={entityKind} /> : <LibraryThumbnailScope><section className="library-list" aria-label="资源列表">
        <header className="library-list-head"><h2>{categories.find(cat=>cat.key===c.category)?.label}</h2>{c.category==="all"&&<small>各类近期摘要；切换到媒体 / 图片 / 视频 / 音频后可按来源、收藏或标签查找。</small>}</header>
        <div className="library-grid">{c.list?.items.map(item=><button type="button" className="library-card" key={`${c.category}:${c.search.trim()}:${c.sourceKind}:${c.favoriteOnly}:${c.tagId}:${props.route.projectId}:${c.pageIndex}:${resourceKey(item.resourceRef)}`} aria-pressed={resourceKey(props.route.resource)===resourceKey(item.resourceRef)} onClick={()=>c.open(item.resourceRef)}><LibraryThumbnail projectId={props.route.projectId} item={item}/><strong title={item.title}>{item.title}</strong><span className="library-card-meta">{item.resourceRef.kind==="reference-set"?"参考集":subtypeLabel(item.subtype)}{sourceOriginLabel(item.sourceOrigin)?<span className={`library-source-badge library-source-badge-${item.sourceOrigin}`}>{sourceOriginLabel(item.sourceOrigin)}</span>:null}</span><time dateTime={item.createdAt}>{new Date(item.createdAt).toLocaleString("zh-CN")}</time></button>)}</div>
        {c.list&&!c.list.items.length&&<p className="library-empty">{c.mediaCategory&&c.sourceKind==="source"?"当前筛选下没有外部导入素材。请先导入图片/视频，或切换来源筛选。":c.mediaCategory&&c.sourceKind==="generated"?"当前筛选下没有生成结果。视频创作请优先使用外部导入素材。":"当前分类 / 搜索没有资源。"}</p>}
        <footer className="library-list-foot">{pager}<small>{c.list?.coverageMessage}</small></footer>
      </section></LibraryThumbnailScope>}
      {showDetail&&<LibraryDetailPanel {...props} controller={c}/>}
    </div>
  </div>;
}