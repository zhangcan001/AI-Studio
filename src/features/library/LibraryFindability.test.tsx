// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,renderHook,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {LibraryPage} from "./LibraryPage";
import {useLibraryController} from "./LibraryController";
const api=vi.hoisted(()=>({list:vi.fn(),tagsList:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{library:api}}));
const tags=[{id:"tag-a",projectId:"a",name:"A tag",createdAt:"date",updatedAt:"date"},{id:"tag-b",projectId:"a",name:"B tag",createdAt:"date",updatedAt:"date"}];
const page=(title:string,nextCursor:unknown=null)=>({items:[{resourceRef:{kind:"asset",id:title},title,subtype:"image",createdAt:"2026-10-01",updatedAt:"2026-10-01",thumbnailAvailable:false}],nextCursor,coverage:"keyset-page",coverageMessage:"当前分类数据库分页结果。"});
const drain=async()=>{await act(async()=>{for(let i=0;i<30;i++)await Promise.resolve();});};
beforeEach(()=>{vi.clearAllMocks();api.tagsList.mockResolvedValue(tags);api.list.mockResolvedValue(page("first"));});
afterEach(()=>{cleanup();vi.useRealTimers();});

it.each(["media","images","videos","audio"] as const)("media-only typed filters visible in %s and empty tags remain usable",async filter=>{
 api.tagsList.mockResolvedValueOnce([]);render(<LibraryPage route={{kind:"library",projectId:"a",filter}} navigate={vi.fn()}/>);
 await screen.findByText("first");expect(screen.getByRole("checkbox",{name:"仅收藏"})).toBeTruthy();expect(screen.getAllByRole("option").map(x=>x.textContent)).toEqual(["全部标签"]);
 expect(api.tagsList).toHaveBeenCalledWith("a");expect(screen.getByText("当前分类数据库分页结果。")).toBeTruthy();
});
it.each(["all","prompts","profiles","reference-sets"] as const)("non-media category %s hides asset filters",async filter=>{
 render(<LibraryPage route={{kind:"library",projectId:"a",filter}} navigate={vi.fn()}/>);await screen.findByText("first");expect(screen.queryByRole("checkbox")).toBeNull();expect(screen.queryByLabelText("标签")).toBeNull();expect(api.tagsList).not.toHaveBeenCalled();
 if(filter==="all"){expect(screen.getByRole("button",{name:"近期资源"})).toBeTruthy();expect(screen.getByText(/切换到媒体/)).toBeTruthy();}
});
it("combined server query resets pages and uses only current cursor; deleted tag clears on refresh",async()=>{
 const cursor={projectId:"a",category:"images",keyword:null,favoriteOnly:false,tagId:null,position:{id:"first",createdAt:"date"}};
 api.list.mockImplementation(async(_p,q)=>page(q.cursor?"second":"first",q.cursor?null:cursor));
 const view=render(<LibraryPage route={{kind:"library",projectId:"a",filter:"images"}} navigate={vi.fn()}/>);await screen.findByText("first");
 fireEvent.click(screen.getByRole("button",{name:"加载更多"}));await screen.findByText("second");
 fireEvent.click(screen.getByRole("checkbox",{name:"仅收藏"}));await waitFor(()=>expect(api.list).toHaveBeenLastCalledWith("a",expect.objectContaining({favoriteOnly:true,cursor:null})));expect(screen.queryByText("second")).toBeNull();
 fireEvent.change(screen.getByLabelText("标签"),{target:{value:"tag-a"}});await waitFor(()=>expect(api.list).toHaveBeenLastCalledWith("a",expect.objectContaining({tagId:"tag-a",favoriteOnly:true,cursor:null})));
 fireEvent.change(screen.getByLabelText("搜索资源名称"),{target:{value:" unique "}});await waitFor(()=>expect(api.list).toHaveBeenLastCalledWith("a",expect.objectContaining({category:"images",keyword:"unique",tagId:"tag-a",favoriteOnly:true,cursor:null,limit:30})));
 api.tagsList.mockResolvedValue([tags[1]]);fireEvent.click(screen.getByRole("button",{name:"刷新"}));await waitFor(()=>expect(api.list).toHaveBeenLastCalledWith("a",expect.objectContaining({tagId:null,cursor:null})));expect((screen.getByLabelText("标签") as HTMLSelectElement).value).toBe("");
 view.unmount();
});
it("pending old keyword/filter responses cannot overwrite current query or project; project resets everything",async()=>{
 let oldResolve:(p:ReturnType<typeof page>)=>void=()=>{};
 api.list.mockImplementation(async(_p,q)=>q.keyword==="old"?new Promise(resolve=>{oldResolve=resolve;}):page(q.keyword||"fresh"));
 api.tagsList.mockImplementation(async(project)=>project==="a"?tags:[{...tags[0],id:"tag-new",projectId:"b",name:"B only"}]);
 const view=renderHook(({project})=>useLibraryController({route:{kind:"library",projectId:project,filter:"images"},navigate:vi.fn()}),{initialProps:{project:"a"}});await drain();
 act(()=>view.result.current.setTagId("tag-a"));await drain();act(()=>view.result.current.setFavoriteOnly(true));await drain();
 act(()=>view.result.current.setSearch("old"));await drain();act(()=>view.result.current.setSearch("new"));await drain();expect(view.result.current.list?.items[0].title).toBe("new");
 await act(async()=>oldResolve(page("STALE")));expect(view.result.current.list?.items[0].title).toBe("new");
 act(()=>view.result.current.setSearch("old"));await drain();view.rerender({project:"b"});await drain();await act(async()=>oldResolve(page("A LEAK")));
 expect(view.result.current.search).toBe("");expect(view.result.current.favoriteOnly).toBe(false);expect(view.result.current.tagId).toBe("");expect(view.result.current.tags.map(t=>t.name)).toEqual(["B only"]);expect(view.result.current.list?.items[0].title).toBe("fresh");
 expect(api.list).toHaveBeenLastCalledWith("b",expect.objectContaining({keyword:null,favoriteOnly:false,tagId:null,cursor:null}));
});
it("invalid query displays understandable error and explicit refresh recovers without retry loops",async()=>{
 api.list.mockRejectedValueOnce({code:"LIBRARY_QUERY_INVALID"});render(<LibraryPage route={{kind:"library",projectId:"a",filter:"images"}} navigate={vi.fn()}/>);await screen.findByRole("alert");expect(api.list).toHaveBeenCalledTimes(1);fireEvent.click(screen.getByRole("button",{name:"刷新"}));await screen.findByText("first");expect(screen.queryByRole("alert")).toBeNull();
});
it("loadMore carries the active normalized favorite/tag/keyword cursor rather than a previous query cursor",async()=>{
 api.list.mockImplementation(async(project,q)=>page(q.cursor?"next":"first",q.cursor?null:{projectId:project,category:q.category,keyword:q.keyword,favoriteOnly:q.favoriteOnly,tagId:q.tagId,position:{id:q.keyword||"default",createdAt:"date"}}));
 const view=renderHook(()=>useLibraryController({route:{kind:"library",projectId:"a",filter:"images"},navigate:vi.fn()}));await drain();
 act(()=>view.result.current.setSearch(" new "));await drain();act(()=>view.result.current.setFavoriteOnly(true));await drain();act(()=>view.result.current.setTagId("tag-b"));await drain();
 const cursor=view.result.current.list!.nextCursor;
 expect(cursor).toMatchObject({projectId:"a",category:"images",keyword:"new",favoriteOnly:true,tagId:"tag-b"});
 act(()=>view.result.current.loadMore());await drain();expect(api.list).toHaveBeenLastCalledWith("a",expect.objectContaining({keyword:"new",favoriteOnly:true,tagId:"tag-b",cursor}));expect(view.result.current.list?.items.map(x=>x.title)).toEqual(["first","next"]);
});
it("pending old favorite and project tag requests cannot publish after ownership changes",async()=>{
 let resolveFavorite:(p:ReturnType<typeof page>)=>void=()=>{};let resolveTags:(t:typeof tags)=>void=()=>{};
 api.list.mockImplementation(async(_p,q)=>q.favoriteOnly?new Promise(resolve=>{resolveFavorite=resolve;}):page("not favorite"));
 const view=renderHook(({project})=>useLibraryController({route:{kind:"library",projectId:project,filter:"images"},navigate:vi.fn()}),{initialProps:{project:"a"}});await drain();
 act(()=>view.result.current.setFavoriteOnly(true));await drain();act(()=>view.result.current.setFavoriteOnly(false));await drain();await act(async()=>resolveFavorite(page("OLD FAVORITE")));expect(view.result.current.list?.items[0].title).toBe("not favorite");
 api.tagsList.mockImplementation(project=>project==="a"?new Promise(resolve=>{resolveTags=resolve;}):Promise.resolve([]));
 act(()=>{void view.result.current.refresh();});await drain();view.rerender({project:"b"});await drain();await act(async()=>resolveTags(tags));expect(view.result.current.tags).toEqual([]);expect(api.list).toHaveBeenLastCalledWith("b",expect.anything());
});
