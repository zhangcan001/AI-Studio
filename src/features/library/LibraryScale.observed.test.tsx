// @vitest-environment jsdom
// Opt-in frontend observation: real React/controller, typed API fixture, not Native IPC.
import {act,cleanup,render} from "@testing-library/react";
import {expect,it,vi} from "vitest";
// @ts-expect-error Node-only opt-in benchmark.
import process from "node:process";
// @ts-expect-error Node-only opt-in benchmark.
import {writeFileSync} from "node:fs";
import {LibraryPage} from "./LibraryPage";
const api=vi.hoisted(()=>({list:vi.fn(),tagsList:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{library:api}}));
it.skipIf(!process.env.AI_STUDIO_UI_BENCHMARK_OUTPUT)("observed Library controlled renderer baseline",async()=>{
 const observations=[];
 const drain=async()=>{await act(async()=>{for(let i=0;i<150;i++)await Promise.resolve();});};
 for(const dataset of [1000,10000])for(const category of ["images","prompts"] as const)for(const pages of [1,5,20]){
  const times=[];let requests=0,bytes=0,dom=0,loaded=0,rss=0;
  for(let repeat=0;repeat<7;repeat++){
   requests=bytes=0;api.tagsList.mockResolvedValue([]);
   api.list.mockImplementation(async(project,q)=>{requests++;const offset=q.cursor?Number(q.cursor.position.id):0;
    const response={items:Array.from({length:30},(_,i)=>({resourceRef:{kind:category==="prompts"?"prompt":"asset",id:String(offset+i)},title:`Resource ${offset+i}`,subtype:category==="prompts"?"prompt":"image",createdAt:"2026-10-01",updatedAt:"2026-10-01",thumbnailAvailable:false})),coverage:"keyset-page",coverageMessage:"数据库分页",nextCursor:offset+30<dataset?{projectId:project,category,keyword:q.keyword,position:{createdAt:"2026-10-01",id:String(offset+30)}}:null};
    bytes+=JSON.stringify(response).length;return response;});
   const start=performance.now();const view=render(<LibraryPage route={{kind:"library",projectId:"owned-a",filter:category}} navigate={()=>{}}/>);await drain();
   for(let page=1;page<pages;page++){await act(async()=>{Array.from(view.container.querySelectorAll("button")).find(b=>b.textContent==="加载更多")!.click();});await drain();}
   times.push(performance.now()-start);dom=view.container.querySelectorAll(".library-card").length;loaded=dom;rss=process.memoryUsage().rss;
   expect(dom).toBe(pages*30);view.unmount();cleanup();
  }
  times.sort((a,b)=>a-b);observations.push({dataset,category,pages,repetitions:7,p50Ms:times[3],p95Ms:times[6],productListCalls:requests,approxFixtureResponseBytes:bytes,domItems:dom,loadedItems:loaded,nodeProcessRss:rss,rendererMemory:"NOT_AVAILABLE",firstVisibleResult:"included in 1-page React/jsdom settle time; not Native paint"});
 }
 // Rapid input / project switch cost and ownership through actual controller.
 for(const category of ["images","prompts"] as const){let calls=0;api.list.mockImplementation(async()=>{calls++;return{items:[],nextCursor:null,coverage:"keyset-page",coverageMessage:"数据库分页"};});const start=performance.now();const view=render(<LibraryPage route={{kind:"library",projectId:"owned-a",filter:category}} navigate={()=>{}}/>);await drain();
  const input=view.getByLabelText("搜索资源名称");const {fireEvent}=await import("@testing-library/react");for(const keyword of ["a","ab","abc","abcd","abcde"]){await act(async()=>{fireEvent.change(input,{target:{value:keyword}});});await drain();}
  view.rerender(<LibraryPage route={{kind:"library",projectId:"owned-b",filter:category}} navigate={()=>{}}/>);await drain();observations.push({category,scenario:"rapid-input-project-switch",productListCalls:calls,durationMs:performance.now()-start});view.unmount();cleanup();}
 writeFileSync(process.env.AI_STUDIO_UI_BENCHMARK_OUTPUT!,JSON.stringify({layer:"React/jsdom with typed API fixtures; no real IPC, browser paint or WebView RSS",observations},null,2));
},120000);
