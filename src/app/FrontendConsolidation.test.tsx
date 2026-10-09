// @vitest-environment jsdom
import { Suspense } from "react";
import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
// @ts-expect-error Test-only Node module.
import { readFileSync } from "node:fs";
// @ts-expect-error Existing build-time guard, not browser code.
import { frontendBoundary } from "../../scripts/frontend-boundary-guard.mjs";
// @ts-expect-error Existing build-time inventory, not browser code.
import { frontendInventory } from "../../scripts/frontend-architecture-inventory.mjs";
import { useProjectTaskRecovery } from "../features/tasks/useProjectTaskRecovery";
import { useShotConsistencyController } from "../features/shots/useShotConsistencyController";
import { useProjectStore } from "../stores/projectStore";
import { useTaskStore } from "../stores/taskStore";
import type { TaskView } from "../types/task";
import { NormalProductPages } from "./NormalProductPages";
import type { ProjectView } from "../types/project";
import type { AppRoute } from "./routes/types";
import { fieldLabel, generatorLabel } from "../product/generatorPresentation";
import type { GeneratorOption } from "../product/types";

const api=vi.hoisted(()=>({ listRecentTasks:vi.fn(),reconcileActiveTasks:vi.fn(),listConsistencyProfiles:vi.fn(),listReferenceSets:vi.fn(),listCostumeVariants:vi.fn(),getShotConsistencyBinding:vi.fn(),getConsistencyScopeBinding:vi.fn(),getShotContextDraft:vi.fn(),replaceShotConsistencyBinding:vi.fn(),replaceConsistencyScopeBinding:vi.fn() }));
vi.mock("../services/tauriClient",()=>api);
vi.mock("../features/create/CreatePage",()=>({CreatePage:({onDirtyChange}:{onDirtyChange:(dirty:boolean)=>void})=><button onClick={()=>onDirtyChange(true)}>create-owned-draft</button>}));
vi.mock("../features/runs/RunsPage",()=>({RunsPage:()=> <p>normal-runs</p>}));
vi.mock("../features/library/LibraryPage",()=>({LibraryPage:()=> <p>normal-library</p>}));
const reportError=vi.fn();
const tasks=(id:string)=>[{id,projectId:id}] as TaskView[];
function deferred<T>() { let resolve!:(value:T)=>void;const promise=new Promise<T>(r=>{resolve=r;});return {promise,resolve}; }
beforeEach(()=>{
  vi.clearAllMocks();useProjectStore.setState({activeProjectId:'a'});useTaskStore.getState().clear();
  api.listRecentTasks.mockResolvedValue([]);api.reconcileActiveTasks.mockResolvedValue({examined:2,succeeded:1,deferred:1,unresolved:0});
  api.listConsistencyProfiles.mockResolvedValue([]);api.listReferenceSets.mockResolvedValue([]);api.listCostumeVariants.mockResolvedValue([]);
});
afterEach(()=>cleanup());

it('phase10_target1 loads once per project and rejects stale task results',async()=>{
  const old=deferred<TaskView[]>();api.listRecentTasks.mockImplementation((id:string)=>id==='a'?old.promise:Promise.resolve(tasks('b')));
  const error=vi.fn();const h=renderHook(({id})=>useProjectTaskRecovery(id,error,api),{initialProps:{id:'a'}});
  useProjectStore.setState({activeProjectId:'b'});h.rerender({id:'b'});
  await waitFor(()=>expect(useTaskStore.getState().recentTasks[0]?.id).toBe('b'));
  await act(async()=>old.resolve(tasks('a')));expect(useTaskStore.getState().recentTasks[0].id).toBe('b');
  expect(api.listRecentTasks.mock.calls).toEqual([['a',10],['b',10]]);expect(error).not.toHaveBeenCalled();
});
it('phase10_target2 reconciles through the original authority and abandons former project callbacks',async()=>{
  const h=renderHook(({id})=>useProjectTaskRecovery(id,reportError,api),{initialProps:{id:'a'}});
  await waitFor(()=>expect(h.result.current.projectContextLoading).toBe(false));
  api.listRecentTasks.mockResolvedValue(tasks('a'));
  await act(async()=>h.result.current.reconcileTasks());expect(h.result.current.recoveryNotice).toContain('已检查 2 个任务');
  const old=deferred<{examined:number,succeeded:number,deferred:number,unresolved:number}>();api.reconcileActiveTasks.mockReturnValue(old.promise);
  let request!:Promise<void>;act(()=>{request=h.result.current.reconcileTasks();});
  useProjectStore.setState({activeProjectId:'b'});api.listRecentTasks.mockResolvedValue(tasks('b'));h.rerender({id:'b'});
  await waitFor(()=>expect(useTaskStore.getState().recentTasks[0]?.id).toBe('b'));
  await act(async()=>{old.resolve({examined:99,succeeded:0,deferred:0,unresolved:0});await request;});
  expect(useTaskStore.getState().recentTasks[0].id).toBe('b');expect(h.result.current.recoveryNotice).toBeNull();expect(h.result.current.reconciling).toBe(false);
});
it('phase10_target3 cancels pending task-store writes on unmount',async()=>{
  const pending=deferred<TaskView[]>();api.listRecentTasks.mockReturnValue(pending.promise);
  const h=renderHook(()=>useProjectTaskRecovery('a',reportError,api));h.unmount();
  await act(async()=>pending.resolve(tasks('a')));expect(useTaskStore.getState().recentTasks).toEqual([]);
});
it('phase10_target4 keeps consistency options scoped through project switches',async()=>{
  const old=deferred<unknown[]>();api.listConsistencyProfiles.mockImplementation((id:string)=>id==='a'?old.promise:Promise.resolve([{id:'b',projectId:'b',profileType:'CHARACTER',name:'B'}]));
  const h=renderHook(({id})=>useShotConsistencyController(id,api),{initialProps:{id:'a'}});h.rerender({id:'b'});
  await waitFor(()=>expect(h.result.current.consistencyProfiles[0]?.id).toBe('b'));
  await act(async()=>old.resolve([{id:'a',projectId:'a',profileType:'CHARACTER',name:'A'}]));expect(h.result.current.consistencyProfiles[0].id).toBe('b');
  expect(api.listReferenceSets.mock.calls).toEqual([['a'],['b']]);h.rerender({id:''});expect(h.result.current.consistencyProfiles).toEqual([]);
});
it('phase10_target5 disposes a pending consistency option projection',async()=>{
  const pending=deferred<unknown[]>();api.listConsistencyProfiles.mockReturnValue(pending.promise);
  const h=renderHook(()=>useShotConsistencyController('a',api));h.unmount();
  await act(async()=>pending.resolve([]));expect(h.result.current.consistencyProfiles).toEqual([]);
});
it('phase10_target6 delegates shot and scope bindings without changing command payloads',async()=>{
  const h=renderHook(()=>useShotConsistencyController('a',api));
  await waitFor(()=>expect(h.result.current.consistencyLoading).toBe(false));
  await h.result.current.loadConsistencyBindingPack({scopeType:'SHOT',scopeId:'s',scopeName:'Shot'});
  await h.result.current.loadConsistencyBindingPack({scopeType:'PROJECT',scopeId:'a',scopeName:'Project'});
  const input={projectId:'a',scopeType:'SHOT',scopeId:'s',profileBindings:[],referenceSetBindings:[]} as Parameters<typeof h.result.current.saveConsistencyBindingPack>[0];
  await h.result.current.saveConsistencyBindingPack(input);
  await h.result.current.loadConsistencyContext({scopeType:'SHOT',scopeId:'s',scopeName:'Shot'},'image');
  expect(api.getShotConsistencyBinding).toHaveBeenCalledWith('a','s');expect(api.getConsistencyScopeBinding).toHaveBeenCalledWith('a','PROJECT','a');
  expect(api.replaceShotConsistencyBinding).toHaveBeenCalledWith(input);expect(api.getShotContextDraft).toHaveBeenCalledWith('a','s','image');
  expect(await h.result.current.loadConsistencyContext({scopeType:'PROJECT',scopeId:'a',scopeName:'Project'},'image')).toBeNull();
});
it('phase10_target7 preserves normal route composition draft handoff and explicit Advanced opt-in',async()=>{
  const project={id:'a'} as ProjectView,dirty=vi.fn(),navigate=vi.fn();let route:AppRoute={kind:'create',projectId:'a',stage:'video'};
  const view=(r:AppRoute)=><Suspense fallback={<p>loading</p>}><NormalProductPages project={project} route={r} navigate={navigate} onDirtyChange={dirty}/></Suspense>;
  const h=render(view(route));fireEvent.click(await screen.findByText('create-owned-draft'));expect(dirty).toHaveBeenCalledWith(true);
  fireEvent.click(screen.getByText('批量创作'));expect(navigate).toHaveBeenCalledWith({...route,surface:'batch'});
  h.rerender(view({...route,surface:'batch'}));expect(screen.queryByText('create-owned-draft')).toBeNull();
  h.rerender(view({...route,stage:'image',surface:'batch'}));expect(await screen.findByText('create-owned-draft')).toBeTruthy();expect(screen.queryByText('批量创作')).toBeNull();
  route={kind:'runs',projectId:'a'};h.rerender(view(route));expect(await screen.findByText('normal-runs')).toBeTruthy();
  h.rerender(view({kind:'library',projectId:'a',filter:'all'}));expect(await screen.findByText('normal-library')).toBeTruthy();
  h.rerender(view({kind:'library',projectId:'a',filter:'advanced-assets'}));expect(screen.queryByText('normal-library')).toBeNull();
});
it('phase10_target8 shares generator presentation without inventing identity or changing labels',()=>{
  const option={name:'wfl_minimax_h3_fixture',mediaKind:'video',version:'1',fields:[{type:'image',key:'first_frame'},{type:'image',key:'last_frame'}]} as GeneratorOption;
  expect(generatorLabel(option,0)).toBe('H3 高质量 · 首尾帧 · 版本 1');
  expect(fieldLabel({type:'integer',key:'duration_seconds',label:'Duration',required:true,default:1})).toBe('时长（秒）');
});
it('phase10_target9 rejects import and root responsibility growth while keeping explicit debt',()=>{
  const manifest=JSON.parse(readFileSync('docs/architecture/phase10-frontend-architecture.json','utf8'));
  expect(frontendBoundary('.',manifest).violations).toEqual([]);
  const missing={...manifest,importDebt:{...manifest.importDebt,'src/app/App.tsx':[]}};
  expect(frontendBoundary('.',missing).violations.some((s:string)=>s.startsWith('feature-import:src/app/App.tsx'))).toBe(true);
  const unscoped={...manifest,frontendSuccessor:{...manifest.frontendSuccessor,'src/services/ipc.ts':'invalid'}};
  expect(frontendBoundary('.',unscoped).violations).toContain('unscoped-successor:src/services/ipc.ts');
  const regressed={...manifest,retiredRootResponsibilities:['<NormalProductPages']};expect(frontendBoundary('.',regressed).violations).toContain('root-responsibility:<NormalProductPages');
  const inventory=frontendInventory('.');expect(inventory.metrics.crossFeature).toBeLessThan(manifest.before.crossFeature);
},15000);
