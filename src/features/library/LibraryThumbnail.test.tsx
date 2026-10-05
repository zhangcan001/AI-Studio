// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { LibraryThumbnail, LibraryThumbnailScope } from "./LibraryThumbnail";
import type { LibraryItem } from "../../product/libraryTypes";
const api=vi.hoisted(()=>({thumbnailGet:vi.fn(),imageGet:vi.fn(),mediaUrl:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{library:api}}));
const observed:Array<{callback:IntersectionObserverCallback;target:Element}>=[];
const create=vi.fn(),revoke=vi.fn();
const item=(id:string,subtype="image",available=true):LibraryItem=>({resourceRef:{kind:"asset",id},title:id,subtype,thumbnailAvailable:available,createdAt:"date",updatedAt:"date"});
function Cards({project="a",page=1,items=Array.from({length:10},(_,i)=>item(String(i)))}:{project?:string;page?:number;items?:LibraryItem[]}){
 return <LibraryThumbnailScope><section className="library-list">{items.map(i=><button key={`${project}:${page}:${i.resourceRef.id}`} onClick={()=>{}}><LibraryThumbnail projectId={project} item={i}/>{i.title}</button>)}</section></LibraryThumbnailScope>;
}
const show=async(indices:number[])=>{await act(async()=>{for(const i of indices)observed[i].callback([{isIntersecting:true} as IntersectionObserverEntry],{} as IntersectionObserver);});};
beforeEach(()=>{
 observed.length=0;vi.clearAllMocks();api.thumbnailGet.mockResolvedValue([1,2,3]);create.mockImplementation(()=>`blob:${create.mock.calls.length}`);
 vi.stubGlobal("IntersectionObserver",class {constructor(private callback:IntersectionObserverCallback){}observe(target:Element){observed.push({callback:this.callback,target});}disconnect(){}unobserve(){}});
 vi.stubGlobal("URL",{createObjectURL:create,revokeObjectURL:revoke});
});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("only 2 visible of 10 eligible cards read thumbnails, three later visible add exactly three reads",async()=>{
 render(<Cards/>);expect(api.thumbnailGet).not.toHaveBeenCalled();await show([0,1]);expect(api.thumbnailGet).toHaveBeenCalledTimes(2);await show([0,1,2,3,4]);expect(api.thumbnailGet).toHaveBeenCalledTimes(5);
 expect(screen.getAllByRole("img")).toHaveLength(5);expect(api.imageGet).not.toHaveBeenCalled();expect(api.mediaUrl).not.toHaveBeenCalled();
});
it("shared page-local limiter caps old plus new in-flight reads at four, discards queued unmounted cards",async()=>{
 let active=0,max=0;const pending:Array<()=>void>=[];
 api.thumbnailGet.mockImplementation(()=>new Promise<number[]>(resolve=>{active++;max=Math.max(max,active);pending.push(()=>{active--;resolve([1]);});}));
 const view=render(<Cards/>);await show([0,1,2,3,4,5,6,7,8,9]);expect(api.thumbnailGet).toHaveBeenCalledTimes(4);
 view.rerender(<Cards page={2}/>);await show([10,11,12,13,14]);expect(api.thumbnailGet).toHaveBeenCalledTimes(4);
 await act(async()=>pending.splice(0,4).forEach(resolve=>resolve()));expect(max).toBe(4);expect(api.thumbnailGet).toHaveBeenCalledTimes(8);expect(create).not.toHaveBeenCalled();
 await act(async()=>pending.splice(0,4).forEach(resolve=>resolve()));await act(async()=>pending.splice(0).forEach(resolve=>resolve()));expect(max).toBe(4);expect(api.thumbnailGet).toHaveBeenCalledTimes(9);
});
it.each(["page","project"] as const)("%s change revokes every URL and pending old thumbnails never publish",async(change)=>{
 const view=render(<Cards/>);await show([0,1]);expect(create).toHaveBeenCalledTimes(2);
 let resolve:(bytes:number[])=>void=()=>{};api.thumbnailGet.mockImplementation(()=>new Promise<number[]>(r=>{resolve=r;}));await show([2]);
 view.rerender(<Cards page={change==="page"?2:1} project={change==="project"?"b":"a"}/>);expect(revoke).toHaveBeenCalledTimes(2);await act(async()=>resolve([4]));expect(create).toHaveBeenCalledTimes(2);expect(screen.queryByRole("img")).toBeNull();
});
it("audio, prompt and unavailable thumbnails never read; video uses managed thumbnail bytes only",async()=>{
 render(<Cards items={[item("missing","image",false),item("audio","audio"),{...item("prompt"),resourceRef:{kind:"prompt",id:"prompt"}},item("video","video")]}/>);expect(observed).toHaveLength(1);await show([0]);expect(api.thumbnailGet).toHaveBeenCalledExactlyOnceWith("a",{kind:"asset",id:"video"});expect(api.imageGet).not.toHaveBeenCalled();expect(api.mediaUrl).not.toHaveBeenCalled();
});
it("one read failure leaves resource clickable and other thumbnails usable; unmount revokes URLs",async()=>{
 api.thumbnailGet.mockRejectedValueOnce(new Error("missing"));const view=render(<Cards/>);await show([0,1]);expect(screen.getByText("缩略图不可用")).toBeTruthy();expect(screen.getByRole("img",{name:"1缩略图"})).toBeTruthy();fireEvent.click(screen.getByText("0").closest("button")!);view.unmount();expect(revoke).toHaveBeenCalledExactlyOnceWith("blob:1");
});
