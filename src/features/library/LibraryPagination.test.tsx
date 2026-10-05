// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { LibraryPage } from "./LibraryPage";
import { useLibraryController } from "./LibraryController";
import { invalidateRuns } from "../../product/runInvalidation";
const api=vi.hoisted(()=>({list:vi.fn(),tagsList:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{library:api}}));
const page=(project:string,q:{cursor?:{position:{id:string}}|null;category:string;keyword:string|null;favoriteOnly:boolean;tagId:string|null})=>{
 const offset=Number(q.cursor?.position.id??0);
 return {items:Array.from({length:30},(_,i)=>({resourceRef:{kind:"asset",id:`${project}-${offset+i}`},title:`${project} item ${offset+i}`,subtype:"image",createdAt:"2026-10-01",updatedAt:"2026-10-01",thumbnailAvailable:false})),coverage:"keyset-page",coverageMessage:"数据库分页",nextCursor:offset<600?{projectId:project,category:q.category,keyword:q.keyword,favoriteOnly:q.favoriteOnly,tagId:q.tagId,position:{id:String(offset+30),createdAt:"date"}}:null};
};
const drain=async()=>{await act(async()=>{for(let i=0;i<50;i++)await Promise.resolve();});};
beforeEach(()=>{vi.clearAllMocks();api.tagsList.mockResolvedValue([{id:"tag",projectId:"a",name:"tag"}]);api.list.mockImplementation(async(p,q)=>page(p,q));});
afterEach(()=>{cleanup();vi.useRealTimers();});

it("1/5/20 bounded pages cost 1/5/20 list calls, previous and next each one; refresh only current page",async()=>{
 render(<LibraryPage route={{kind:"library",projectId:"a",filter:"images"}} navigate={vi.fn()}/>);await drain();
 for(let n=1;n<=20;n++){
  expect(api.list).toHaveBeenCalledTimes(n);expect(document.querySelectorAll(".library-card")).toHaveLength(30);expect(screen.getByText(`第 ${n} 页`)).toBeTruthy();
  if(n<20){fireEvent.click(screen.getByRole("button",{name:"下一页"}));await drain();}
 }
 expect(api.tagsList).toHaveBeenCalledTimes(1);
 fireEvent.click(screen.getByRole("button",{name:"上一页"}));await drain();expect(api.list).toHaveBeenCalledTimes(21);expect(screen.getByText("第 19 页")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"下一页"}));await drain();expect(api.list).toHaveBeenCalledTimes(22);
 fireEvent.click(screen.getByRole("button",{name:"刷新"}));await drain();expect(api.list).toHaveBeenCalledTimes(23);expect(api.list.mock.lastCall?.[1].cursor.position.id).toBe("570");expect(api.tagsList).toHaveBeenCalledTimes(2);
});
it("page20 polling and invalidation each query current page once; polling does not fetch tags",async()=>{
 vi.useFakeTimers();const view=renderHook(()=>useLibraryController({route:{kind:"library",projectId:"a",filter:"images"},navigate:vi.fn()}));await drain();
 for(let n=1;n<20;n++){act(()=>view.result.current.nextPage());await drain();}
 api.list.mockClear();api.tagsList.mockClear();await act(async()=>vi.advanceTimersByTime(5000));await drain();
 expect(api.list).toHaveBeenCalledTimes(1);expect(api.list.mock.lastCall?.[1].cursor.position.id).toBe("570");expect(api.tagsList).not.toHaveBeenCalled();
 act(()=>invalidateRuns("a"));await act(async()=>vi.advanceTimersByTime(150));await drain();expect(api.list).toHaveBeenCalledTimes(2);expect(api.tagsList).toHaveBeenCalledTimes(1);
});
it.each(["keyword","favorite","tag","category","project"] as const)("%s change clears cursor stack and starts one fresh first page",async(change)=>{
 const view=renderHook(({project,category})=>useLibraryController({route:{kind:"library",projectId:project,filter:category},navigate:vi.fn()}),{initialProps:{project:"a",category:"images"}});await drain();act(()=>view.result.current.nextPage());await drain();
 if(change==="keyword")act(()=>view.result.current.setSearch("new"));
 if(change==="favorite")act(()=>view.result.current.setFavoriteOnly(true));
 if(change==="tag")act(()=>view.result.current.setTagId("tag"));
 if(change==="category")view.rerender({project:"a",category:"videos"});
 if(change==="project")view.rerender({project:"b",category:"images"});
 await drain();expect(view.result.current.pageIndex).toBe(0);expect(view.result.current.canPrevious).toBe(false);expect(api.list.mock.lastCall?.[1].cursor).toBeNull();
});
it("pending next page cannot publish after query/project changes or a newer previous-page request",async()=>{
 const view=renderHook(({project})=>useLibraryController({route:{kind:"library",projectId:project,filter:"images"},navigate:vi.fn()}),{initialProps:{project:"a"}});await drain();act(()=>view.result.current.nextPage());await drain();
 let resolveOld:(value:ReturnType<typeof page>)=>void=()=>{};
 api.list.mockImplementation((p,q)=>q.cursor?.position.id==="60"?new Promise(resolve=>{resolveOld=resolve;}):Promise.resolve(page(p,q)));
 act(()=>view.result.current.nextPage());await drain();act(()=>view.result.current.previousPage());await drain();await act(async()=>resolveOld(page("a",{category:"images",keyword:null,favoriteOnly:false,tagId:null,cursor:{position:{id:"60"}}})));
 expect(view.result.current.pageIndex).toBe(0);expect(view.result.current.list?.items[0].title).toBe("a item 0");
 act(()=>view.result.current.nextPage());await drain();act(()=>view.result.current.nextPage());await drain();view.rerender({project:"b"});await drain();await act(async()=>resolveOld(page("a",{category:"images",keyword:null,favoriteOnly:false,tagId:null,cursor:{position:{id:"60"}}})));
 expect(view.result.current.pageIndex).toBe(0);expect(view.result.current.list?.items[0].title).toBe("b item 0");
});
it.each(["all","profiles","reference-sets"] as const)("%s summary/complete-category never shows navigation",async(filter)=>{
 api.list.mockResolvedValue({items:[],coverage:filter==="all"?"recent-summary":"complete-category",coverageMessage:"bounded",nextCursor:null});render(<LibraryPage route={{kind:"library",projectId:"a",filter}} navigate={vi.fn()}/>);await drain();expect(screen.queryByLabelText("资源分页")).toBeNull();
});
it("poll cannot cancel pending navigation; failed navigation never commits page history and retry recovers",async()=>{
 vi.useFakeTimers();const view=renderHook(()=>useLibraryController({route:{kind:"library",projectId:"a",filter:"images"},navigate:vi.fn()}));await drain();
 let reject:(error:Error)=>void=()=>{};api.list.mockImplementationOnce(()=>new Promise((_resolve,r)=>{reject=r;}));
 act(()=>view.result.current.nextPage());await drain();await act(async()=>vi.advanceTimersByTime(5000));await drain();expect(api.list).toHaveBeenCalledTimes(2);
 await act(async()=>reject(new Error("offline")));expect(view.result.current.pageIndex).toBe(0);expect(view.result.current.canPrevious).toBe(false);
 act(()=>view.result.current.nextPage());await drain();expect(view.result.current.pageIndex).toBe(1);expect(api.list.mock.lastCall?.[1].cursor.position.id).toBe("30");
});
it("page2 pending then keyword changes: old response and start cursor cannot leak",async()=>{
 const view=renderHook(()=>useLibraryController({route:{kind:"library",projectId:"a",filter:"images"},navigate:vi.fn()}));await drain();
 let resolve:(value:ReturnType<typeof page>)=>void=()=>{};api.list.mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));act(()=>view.result.current.nextPage());await drain();
 act(()=>view.result.current.setSearch("new"));await drain();await act(async()=>resolve(page("a",{category:"images",keyword:null,favoriteOnly:false,tagId:null,cursor:{position:{id:"30"}}})));
 expect(view.result.current.pageIndex).toBe(0);expect(view.result.current.list?.items[0].title).toBe("a item 0");expect(api.list.mock.lastCall?.[1]).toMatchObject({keyword:"new",cursor:null});
});
