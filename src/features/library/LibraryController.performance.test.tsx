// @vitest-environment jsdom
import {act,cleanup,renderHook} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {useLibraryController} from "./LibraryController";
import {useRunsController} from "../runs/RunsController";
import {invalidateRuns,subscribeRunInvalidation} from "../../product/runInvalidation";
const state=vi.hoisted(()=>({subscribed:0,unsubscribed:0,active:0,library:{list:vi.fn(),get:vi.fn(),relationsGet:vi.fn(),versionsGet:vi.fn(),resourceEdit:vi.fn()},run:{list:vi.fn()}}));
vi.mock("../../product/client",()=>({productClient:{library:state.library,run:state.run}}));
vi.mock("../../product/runInvalidation",async original=>{
 const actual=await original<typeof import("../../product/runInvalidation")>();
 return {...actual,subscribeRunInvalidation:(listener:(project:string)=>void)=>{
  state.subscribed++;state.active++;const off=actual.subscribeRunInvalidation(listener);let live=true;
  return()=>{expect(live).toBe(true);live=false;state.unsubscribed++;state.active--;off();};
 }};
});
afterEach(()=>{cleanup();vi.useRealTimers();});
const drain=async()=>{await act(async()=>{for(let i=0;i<20;i++)await Promise.resolve();});};
it("phase12_case10 one edit refresh preserves external project poll freshness and balanced subscriptions",async()=>{
 vi.useFakeTimers();state.subscribed=state.unsubscribed=state.active=0;
 const resource={kind:"prompt",id:"prompt-a"} as const;
 state.library.list.mockImplementation(async project=>({items:[],nextCursor:null,coverage:project,coverageMessage:project}));
 state.library.get.mockImplementation(async project=>({kind:"prompt",prompt:{id:resource.id,projectId:project,name:"fresh",versions:[]}}));
 state.library.relationsGet.mockResolvedValue([]);state.library.versionsGet.mockResolvedValue({kind:"prompt",versions:[]});state.library.resourceEdit.mockResolvedValue({});
 state.run.list.mockResolvedValue({items:[],nextCursor:null,coverage:"recent"});
 const navigate=vi.fn();const view=renderHook(({project})=>useLibraryController({route:{kind:"library",projectId:project,filter:"prompts",resource},navigate}),{initialProps:{project:"project-a"}});
 await drain();expect(state.active).toBe(1);expect(vi.getTimerCount()).toBe(1);
 const other=vi.fn();const off=subscribeRunInvalidation(other);
 for(const mock of Object.values(state.library))mock.mockClear();
 await act(async()=>{await view.result.current.edit({kind:"prompt",id:resource.id,text:"new",modelVersionId:null});});
 await act(async()=>{await vi.advanceTimersByTimeAsync(200);});await drain();
 expect(state.library.resourceEdit).toHaveBeenCalledTimes(1);
 for(const name of ["list","get","relationsGet","versionsGet"] as const)expect(state.library[name]).toHaveBeenCalledTimes(1);
 expect(other).toHaveBeenCalledWith("project-a", undefined);expect(view.result.current.detail).toMatchObject({prompt:{projectId:"project-a"}});
 // The next real event must not be swallowed by a stale self-origin token.
 act(()=>invalidateRuns("project-a"));await act(async()=>{await vi.advanceTimersByTimeAsync(150);});await drain();expect(state.library.list).toHaveBeenCalledTimes(2);
 view.rerender({project:"project-b"});await drain();expect(view.result.current.detail).toMatchObject({prompt:{projectId:"project-b"}});
 const before=state.library.list.mock.calls.length;act(()=>invalidateRuns("project-a"));await act(async()=>{await vi.advanceTimersByTimeAsync(200);});expect(state.library.list).toHaveBeenCalledTimes(before);
 await act(async()=>{await vi.advanceTimersByTimeAsync(4800);});await drain();expect(state.library.list.mock.calls.length).toBe(before+1);expect(state.library.list).toHaveBeenLastCalledWith("project-b",expect.anything());
 off();view.unmount();expect(state.active).toBe(0);expect(vi.getTimerCount()).toBe(0);
 // Both real controllers repeatedly mount/unmount with the real notification Set.
 for(let i=0;i<20;i++){
  const lib=renderHook(()=>useLibraryController({route:{kind:"library",projectId:"project-a"},navigate}));
  const runs=renderHook(()=>useRunsController({route:{kind:"runs",projectId:"project-a"},navigate}));
  await drain();expect(state.active).toBe(2);expect(vi.getTimerCount()).toBe(2);
  lib.unmount();runs.unmount();expect(state.active).toBe(0);expect(vi.getTimerCount()).toBe(0);
 }
 expect(state.subscribed).toBe(state.unsubscribed);
 const calls=state.library.list.mock.calls.length;act(()=>invalidateRuns("project-a"));await act(async()=>{await vi.advanceTimersByTimeAsync(5000);});expect(state.library.list).toHaveBeenCalledTimes(calls);
});

it("slow Library reads remain single-flight across polling ticks",async()=>{
 vi.useFakeTimers();state.library.list.mockReset();
 let complete!:(value:{items:never[];nextCursor:null;coverage:string})=>void;
 state.library.list.mockImplementation(()=>new Promise(resolve=>{complete=resolve;}));
 const view=renderHook(()=>useLibraryController({route:{kind:"library",projectId:"project-a",filter:"prompts"},navigate:vi.fn()}));
 await drain();expect(state.library.list).toHaveBeenCalledTimes(1);
 await act(async()=>{await vi.advanceTimersByTimeAsync(15000);});await drain();
 expect(state.library.list).toHaveBeenCalledTimes(1);
 await act(async()=>complete({items:[],nextCursor:null,coverage:"complete-category"}));await drain();
 expect(view.result.current.loading).toBe(false);
});

it("a rejected background read releases polling and preserves the current projection until recovery",async()=>{
 vi.useFakeTimers();state.library.list.mockReset();
 const list={items:[],nextCursor:null,coverage:"complete-category"};state.library.list.mockResolvedValue(list);
 const view=renderHook(()=>useLibraryController({route:{kind:"library",projectId:"project-a",filter:"prompts"},navigate:vi.fn()}));await drain();
 state.library.list.mockRejectedValueOnce(new Error("temporary read error"));
 await act(async()=>{await vi.advanceTimersByTimeAsync(5000);});await drain();
 expect(view.result.current.queryError).toBeTruthy();expect(view.result.current.list).toEqual(list);expect(view.result.current.loading).toBe(false);
 await act(async()=>{await vi.advanceTimersByTimeAsync(5000);});await drain();
 expect(state.library.list).toHaveBeenCalledTimes(3);expect(view.result.current.queryError).toBeUndefined();expect(view.result.current.list).toEqual(list);
});

it("an obsolete poll cannot unlock or overwrite a newer explicit refresh",async()=>{
 vi.useFakeTimers();state.library.list.mockReset();
 const list={items:[],nextCursor:null,coverage:"complete-category"};state.library.list.mockResolvedValue(list);
 const view=renderHook(()=>useLibraryController({route:{kind:"library",projectId:"project-a",filter:"prompts"},navigate:vi.fn()}));await drain();
 let old!:(value:typeof list)=>void,newer!:(value:typeof list)=>void;
 state.library.list.mockImplementationOnce(()=>new Promise(resolve=>{old=resolve;}));
 await act(async()=>{await vi.advanceTimersByTimeAsync(5000);});await drain();
 state.library.list.mockImplementationOnce(()=>new Promise(resolve=>{newer=resolve;}));
 let refreshing!:Promise<void>;act(()=>{refreshing=view.result.current.refresh();});await drain();
 expect(view.result.current.loading).toBe(true);
 await act(async()=>old({...list,coverage:"obsolete"}));await drain();
 await act(async()=>{await vi.advanceTimersByTimeAsync(5000);});await drain();
 expect(state.library.list).toHaveBeenCalledTimes(3);expect(view.result.current.loading).toBe(true);expect(view.result.current.list).toEqual(list);
 const fresh={...list,coverage:"fresh"};await act(async()=>{newer(fresh);await refreshing;});await drain();
 expect(view.result.current.loading).toBe(false);expect(view.result.current.list).toEqual(fresh);
 await act(async()=>{await vi.advanceTimersByTimeAsync(5000);});await drain();expect(state.library.list).toHaveBeenCalledTimes(4);
});

it("a real Library invalidation supersedes a pending poll instead of being swallowed by the poll lock",async()=>{
 vi.useFakeTimers();state.library.list.mockReset();
 const list={items:[],nextCursor:null,coverage:"complete-category"};state.library.list.mockResolvedValue(list);
 const view=renderHook(()=>useLibraryController({route:{kind:"library",projectId:"project-a",filter:"prompts"},navigate:vi.fn()}));await drain();
 let old!:(value:typeof list)=>void;state.library.list.mockImplementationOnce(()=>new Promise(resolve=>{old=resolve;}));
 await act(async()=>{await vi.advanceTimersByTimeAsync(5000);});await drain();
 const fresh={...list,coverage:"after-invalidation"};state.library.list.mockResolvedValue(fresh);
 act(()=>invalidateRuns("project-a"));await act(async()=>{await vi.advanceTimersByTimeAsync(150);});await drain();
 expect(state.library.list).toHaveBeenCalledTimes(3);expect(view.result.current.list).toEqual(fresh);
 await act(async()=>old(list));await drain();expect(view.result.current.list).toEqual(fresh);
});

it("background polling retains the displayed Library without repeating the initial loading state",async()=>{
 vi.useFakeTimers();state.library.list.mockReset();
 const list={items:[],nextCursor:null,coverage:"complete-category"};
 state.library.list.mockResolvedValue(list);
 const view=renderHook(()=>useLibraryController({route:{kind:"library",projectId:"project-a",filter:"prompts"},navigate:vi.fn()}));
 await drain();expect(view.result.current.loading).toBe(false);
 let complete!:(value:typeof list)=>void;
 state.library.list.mockImplementationOnce(()=>new Promise(resolve=>{complete=resolve;}));
 await act(async()=>{await vi.advanceTimersByTimeAsync(5000);});await drain();
 expect(state.library.list).toHaveBeenCalledTimes(2);
 expect(view.result.current.loading).toBe(false);
 expect(view.result.current.list).toEqual(list);
 await act(async()=>complete(list));await drain();
 expect(view.result.current.loading).toBe(false);
});
