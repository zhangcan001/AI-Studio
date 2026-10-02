import { useLibraryController, type LibraryProps } from "./LibraryController";
import { LibraryDetailPanel } from "./LibraryDetail";
import { categories, resourceKey, subtypeLabel } from "./libraryModel";
import "./LibraryPage.css";
export function LibraryPage(props:LibraryProps) {
  const c=useLibraryController(props);
  return <div className="library-page"><header className="library-header"><h1>资源库</h1><label>搜索名称<input type="search" aria-label="搜索资源名称" value={c.search} onChange={e=>c.setSearch(e.target.value)}/></label><button type="button" onClick={()=>void c.refresh()}>刷新</button><details><summary>高级</summary><button type="button" onClick={()=>props.navigate({...props.route,filter:"advanced-assets"})}>素材导入 / 高级设定</button><button type="button" onClick={()=>props.navigate({...props.route,filter:"advanced-prompts"})}>高级提示词编辑器</button></details></header>
    {c.queryError&&<p role="alert">{c.queryError}</p>}{c.actionError&&<p role="alert">{c.actionError}</p>}{c.notice&&<p role="status">{c.notice}</p>}{c.loading&&<p role="status">正在读取资源…</p>}
    <div className="library-layout"><nav aria-label="资源分类">{categories.map(cat=><button type="button" key={cat.key} aria-pressed={cat.key===c.category} onClick={()=>props.navigate({kind:"library",projectId:props.route.projectId,filter:cat.key})}>{cat.label}</button>)}</nav><section className="library-list" aria-label="资源列表"><h2>{categories.find(cat=>cat.key===c.category)?.label}</h2>{c.list?.items.map(item=><button type="button" className="library-card" key={resourceKey(item.resourceRef)} aria-pressed={resourceKey(props.route.resource)===resourceKey(item.resourceRef)} onClick={()=>c.open(item.resourceRef)}><strong>{item.title}</strong><span>{item.resourceRef.kind==="reference-set"?"参考集":subtypeLabel(item.subtype)}</span><time dateTime={item.createdAt}>{new Date(item.createdAt).toLocaleString("zh-CN")}</time></button>)}{c.list&&!c.list.items.length&&<p>当前分类 / 搜索没有资源。</p>}<small>{c.list?.coverageMessage}</small>{c.list?.nextCursor&&<button type="button" onClick={c.loadMore}>加载更多</button>}</section><LibraryDetailPanel {...props} controller={c}/></div>
  </div>;
}
