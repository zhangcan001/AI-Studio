// @vitest-environment jsdom
import { useLayoutEffect } from "react";
import { useFolderBatchImportController } from "./useFolderBatchImportController";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FolderBatchImportPanel, FolderBatchImportDialog } from "./FolderBatchImportPanel";
import type { FolderImportPreview, FolderImportResult } from "../../types/folderBatchImport";
const api=vi.hoisted(()=>({pick:vi.fn(),rescan:vi.fn(),discard:vi.fn(),commit:vi.fn(),generators:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{creation:{generatorsList:api.generators},folderImport:{pick:api.pick,rescan:api.rescan,discard:api.discard,commit:api.commit}}}));
function preview(id="session",bad=true):FolderImportPreview{return {sessionId:id,previewVersion:1,nameSuffix:null,displayRootName:"fixture",manifestSha256:"abc",formatSource:"manifest",shotCount:3,validCount:bad?2:3,errorCount:bad?1:0,warningCount:0,maxQueueItems:100,createQueueDefault:true,characters:[],errors:[],warnings:[],rows:[0,1,2].map(i=>({rowKey:`r${i}`,rowNumber:i+1,manifestId:null,mode:i===0?"firstLast":"reference",name:`镜头${i}`,promptPreview:"真实原文",durationSeconds:6,width:1280,height:768,generatorLabel:"Exact",firstFrame:i===0?"a.png":null,lastFrame:i===0?"b.png":null,referenceCount:2,entityNames:[],errors:i===2&&bad?[{code:"FILE_NOT_FOUND",message:"图片缺失"}]:[],warnings:[]}))};}
function result(outcome:FolderImportResult["outcome"]="completed"):FolderImportResult{return {outcome,failure:outcome==="completed"?null:{stage:"C3",code:"PROMPT_SAVE_FAILED",message:"写入失败"},compensationErrors:[],createdShotIds:["s1","s2"],reusedEntityIds:[],createdEntityIds:[],importedAssetIds:["a","b"],batchId:"batch",itemCount:2,autoStarted:false,residual:{unverifiedFileCleanup:false,shotIds:[],entityIds:[],assetIds:[],batchId:null},warnings:[]};}
function deferred<T>(){let resolve!:(v:T)=>void;let reject!:(e:unknown)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};}
beforeEach(()=>{vi.clearAllMocks();api.generators.mockResolvedValue([]);api.pick.mockResolvedValue(preview());api.rescan.mockResolvedValue({...preview(),previewVersion:2});api.discard.mockResolvedValue(undefined);api.commit.mockResolvedValue(result());HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};});
afterEach(cleanup);
async function pick(){fireEvent.click(screen.getByRole("button",{name:"选择文件夹"}));await screen.findByText("fixture");}
describe("folder import preview and authority",()=>{
 it("legacy defaults use an explicit catalog identity and require a server rescan",async()=>{
  const p=preview("legacy",false);p.formatSource="h3-batch";p.rows=p.rows.slice(0,1);p.shotCount=1;p.validCount=0;p.errorCount=1;p.rows[0].errors=[{code:"GENERATOR_UNAVAILABLE",message:"请选择生成器"}];
  api.pick.mockResolvedValue(p);api.generators.mockResolvedValue([{selectionRef:"exact-selection",name:"Installed",version:"1",mode:"I2V",mediaKind:"video",availability:true,recommended:false,availabilityReason:null,fields:[],resolutionPresets:[{id:"official-preset",label:"Official",width:1280,height:768}]}]);
  api.rescan.mockResolvedValue({...p,previewVersion:2,validCount:1,rows:[{...p.rows[0],errors:[]}]});
  render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()}/>);await pick();
  const select=await screen.findByRole("combobox",{name:"生成器"});await screen.findByRole("option",{name:/Installed/});fireEvent.change(select,{target:{value:"exact-selection"}});expect(screen.getByRole("button",{name:"确认导入"})).toHaveProperty("disabled",true);
  fireEvent.click(screen.getByRole("button",{name:"重新校验"}));await waitFor(()=>expect(screen.getByRole("button",{name:"确认导入"})).toHaveProperty("disabled",false));expect(api.rescan).toHaveBeenCalledWith("legacy",{nameSuffix:null,legacy:{selectionRef:"exact-selection",durationSeconds:6,width:1280,height:768}});
  fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入完成");expect(api.commit.mock.calls[0][0]).toMatchObject({sessionId:"legacy",previewVersion:2,autoStart:false});
 });
 it("does not offer a queue which successful compensation already deleted",async()=>{
  api.commit.mockResolvedValue(result("compensated"));render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()} onOpenQueue={vi.fn()}/>);await pick();fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入失败，本次新增的数据已撤回");expect(screen.queryByRole("button",{name:"查看生产队列"})).toBeNull();
 });
 it("fences project ownership before passive cleanup can clear the old preview",async()=>{
  const observations:Array<{visible:string|undefined;allowed:boolean}>=[];
  function Probe({project,submit}:{project:string;submit:boolean}) {const c=useFolderBatchImportController(project,vi.fn());useLayoutEffect(()=>{if(submit){observations.push({visible:c.preview?.sessionId,allowed:c.canCommit});void c.commit();}},[project,submit]);return <><button onClick={()=>void c.pick()}>选择文件夹</button><p>{c.preview?.displayRootName}</p></>;}
  const view=render(<Probe project="p" submit={false}/>);await pick();view.rerender(<Probe project="q" submit={true}/>);expect(observations).toEqual([{visible:undefined,allowed:false}]);expect(api.commit).not.toHaveBeenCalled();
 });
 it("labels prop and composition entities in the import plan",async()=>{
  const p=preview("kinds",false);p.characters=[{name:"调阅签",kind:"prop",action:"create",imageCount:1,existingEntityId:null},{name:"剧情01",kind:"composition",action:"create",imageCount:1,existingEntityId:null}];api.pick.mockResolvedValue(p);
  render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()}/>);await pick();
  expect(screen.getByText("调阅签 · 道具 · 新建，1 张图")).toBeTruthy();expect(screen.getByText("剧情01 · 构图 · 新建，1 张图")).toBeTruthy();
 });
 it("commits exact valid row keys and starts nothing by default",async()=>{
  const imported=vi.fn();const queue=vi.fn();render(<FolderBatchImportPanel projectId="p" onImported={imported} onCancel={vi.fn()} onOpenQueue={queue}/>);await pick();
  expect(screen.getByRole("checkbox",{name:"导入 镜头2"})).toHaveProperty("disabled",true);expect(screen.getByText("图片缺失")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入完成");
  expect(api.commit).toHaveBeenCalledExactlyOnceWith({sessionId:"session",previewVersion:1,manifestSha256:"abc",includeRowKeys:["r0","r1"],createQueue:true,autoStart:false,nameSuffix:null});
  expect(imported).toHaveBeenCalledOnce();fireEvent.click(screen.getByRole("button",{name:"查看生产队列"}));expect(queue).toHaveBeenCalledWith("batch");expect(screen.queryByRole("button",{name:"确认导入"})).toBeNull();
 });
 it("requires a new formal preview after editing suffix; never unlocks errors locally",async()=>{
  const scan=deferred<FolderImportPreview>();api.rescan.mockReturnValue(scan.promise);render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()}/>);await pick();
  fireEvent.change(screen.getByLabelText("镜头名称后缀"),{target:{value:" v2"}});expect(screen.getByRole("button",{name:"确认导入"})).toHaveProperty("disabled",true);expect(screen.getByRole("checkbox",{name:"导入 镜头2"})).toHaveProperty("disabled",true);
  fireEvent.click(screen.getByRole("button",{name:"重新校验"}));expect(api.rescan).toHaveBeenCalledWith("session",{nameSuffix:" v2"});
  await act(async()=>scan.resolve({...preview("session",false),previewVersion:2,nameSuffix:" v2"}));expect(screen.getByRole("checkbox",{name:"导入 镜头2"})).toHaveProperty("disabled",false);fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入完成");expect(api.commit.mock.calls[0][0]).toMatchObject({previewVersion:2,nameSuffix:" v2",includeRowKeys:["r0","r1","r2"]});
 });
 it("reports partial cleanup and blocks double click/repeated writes",async()=>{
  const save=deferred<FolderImportResult>();api.commit.mockReturnValue(save.promise);render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()}/>);await pick();const b=screen.getByRole("button",{name:"确认导入"});fireEvent.click(b);fireEvent.click(b);expect(api.commit).toHaveBeenCalledOnce();expect(screen.getByRole("button",{name:"关闭"})).toHaveProperty("disabled",true);
  const r=result("partial");r.residual.assetIds=["retained"];r.compensationErrors=[{target:"asset",id:"retained",message:"仍被引用"}];await act(async()=>save.resolve(r));expect(screen.getByText("导入未完成，有部分数据未能撤回")).toBeTruthy();expect(screen.getByText("写入失败")).toBeTruthy();expect(screen.getByText("素材：仍被引用")).toBeTruthy();expect(screen.getByText(/未撤回：镜头 0 个、角色\/场景 0 个、素材 1 个/)).toBeTruthy();expect(screen.queryByRole("button",{name:"确认导入"})).toBeNull();
 });
 it("keeps completed result after list refresh error and never retries automatically",async()=>{
  render(<FolderBatchImportPanel projectId="p" onImported={()=>Promise.reject(Error("refresh"))} onCancel={vi.fn()}/>);await pick();fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入完成");await screen.findByText(/列表刷新失败/);expect(api.commit).toHaveBeenCalledOnce();
 });
 it("requires explicit GPU opt-in and honors the backend single-batch cap",async()=>{
  api.pick.mockResolvedValue({...preview("s",false),maxQueueItems:2});render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()}/>);await pick();expect(screen.getByRole("button",{name:"确认导入"})).toHaveProperty("disabled",true);fireEvent.click(screen.getByRole("checkbox",{name:"导入 镜头2"}));
  const start=screen.getByRole("checkbox",{name:/导入后立即开始生产/});expect(start).toHaveProperty("checked",false);fireEvent.click(start);expect(screen.getByRole("button",{name:"导入并开始生产"})).toHaveProperty("disabled",false);fireEvent.click(screen.getByRole("checkbox",{name:"创建待启动批次"}));expect(start).toHaveProperty("checked",false);expect(start).toHaveProperty("disabled",true);fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入完成");expect(api.commit.mock.calls[0][0]).toMatchObject({createQueue:false,autoStart:false});
 });
 it("discards a late pick after project switch and cannot overwrite current preview",async()=>{
  const old=deferred<FolderImportPreview>();api.pick.mockReturnValueOnce(old.promise).mockResolvedValueOnce(preview("new"));const props={onImported:vi.fn(),onCancel:vi.fn()};const view=render(<FolderBatchImportPanel projectId="p" {...props}/>);fireEvent.click(screen.getByRole("button",{name:"选择文件夹"}));view.rerender(<FolderBatchImportPanel projectId="q" {...props}/>);await pick();await act(async()=>old.resolve({...preview("old"),displayRootName:"OLD"}));expect(screen.queryByText("OLD")).toBeNull();expect(api.discard).toHaveBeenCalledWith("old");fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入完成");expect(api.commit.mock.calls[0][0].sessionId).toBe("new");
 });
 it("late rescan after unmount never calls imported or commit",async()=>{
  const scan=deferred<FolderImportPreview>();api.rescan.mockReturnValue(scan.promise);const imported=vi.fn();const view=render(<FolderBatchImportPanel projectId="p" onImported={imported} onCancel={vi.fn()}/>);await pick();fireEvent.click(screen.getByRole("button",{name:"重新校验"}));view.unmount();await act(async()=>scan.resolve({...preview(),previewVersion:2}));expect(api.discard).toHaveBeenCalledWith("session");expect(imported).not.toHaveBeenCalled();expect(api.commit).not.toHaveBeenCalled();
 });
 it("dialog uses real dialog semantics and Escape follows the enabled close action",async()=>{
  const cancel=vi.fn();render(<FolderBatchImportDialog projectId="p" onImported={vi.fn()} onCancel={cancel}/>);const d=screen.getByRole("dialog",{name:"文件夹批量导入"});expect(d).toHaveProperty("open",true);fireEvent(d,new Event("cancel",{bubbles:false,cancelable:true}));expect(cancel).toHaveBeenCalledOnce();
 });
});
