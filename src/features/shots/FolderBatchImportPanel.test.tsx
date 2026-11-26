// @vitest-environment jsdom
import { useLayoutEffect } from "react";
import { useFolderBatchImportController } from "./useFolderBatchImportController";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FolderBatchImportPanel, FolderBatchImportDialog } from "./FolderBatchImportPanel";
import type { FolderImportPreview, FolderImportResult } from "../../types/folderBatchImport";
import { subscribeRunInvalidation } from "../../product/runInvalidation";
import { subscribeVideoInputInvalidation } from "../../product/videoInputInvalidation";
import { STORY_ENTITIES_CHANGED } from "../../types/storyEntity";
const api=vi.hoisted(()=>({pick:vi.fn(),rescan:vi.fn(),discard:vi.fn(),commit:vi.fn(),generators:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{creation:{generatorsList:api.generators},folderImport:{pick:api.pick,rescan:api.rescan,discard:api.discard,commit:api.commit}}}));
function preview(id="session",bad=true):FolderImportPreview{return {sessionId:id,previewVersion:1,nameSuffix:null,displayRootName:"fixture",manifestSha256:"abc",formatSource:"manifest",shotCount:3,validCount:bad?2:3,errorCount:bad?1:0,warningCount:0,maxQueueItems:100,createQueueDefault:true,characters:[],errors:[],warnings:[],rows:[0,1,2].map(i=>({rowKey:`r${i}`,rowNumber:i+1,manifestId:null,mode:i===0?"firstLast":"reference",name:`镜头${i}`,prompt:"真实完整原文",promptSections:{style:null,summary:null,soundscape:null,music:null},referencePreview:["a.png","b.png"],promptPreview:"真实原文",durationSeconds:6,width:1280,height:768,generatorLabel:"Exact",firstFrame:i===0?"a.png":null,lastFrame:i===0?"b.png":null,referenceCount:2,entityNames:[],errors:i===2&&bad?[{code:"FILE_NOT_FOUND",message:"图片缺失"}]:[],warnings:[]}))};}
function result(outcome:FolderImportResult["outcome"]="completed"):FolderImportResult{return {outcome,failure:outcome==="completed"?null:{stage:"C3",code:"PROMPT_SAVE_FAILED",message:"写入失败"},compensationErrors:[],createdRows:[{rowKey:"r0",shotId:"s1",name:"镜头0"},{rowKey:"r1",shotId:"s2",name:"镜头1"}],createdShotIds:["s1","s2"],reusedEntityIds:[],createdEntityIds:[],importedAssetIds:["a","b"],batchId:"batch",itemCount:2,autoStarted:false,residual:{unverifiedFileCleanup:false,shotIds:[],entityIds:[],assetIds:[],batchId:null},warnings:[]};}
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
  fireEvent.click(screen.getByRole("button",{name:"重新校验"}));await waitFor(()=>expect(screen.getByRole("checkbox",{name:"导入 镜头0"})).toHaveProperty("disabled",false));expect(screen.getByRole("checkbox",{name:"导入 镜头0"})).toHaveProperty("checked",false);fireEvent.click(screen.getByRole("checkbox",{name:"导入 镜头0"}));await waitFor(()=>expect(screen.getByRole("button",{name:"确认导入"})).toHaveProperty("disabled",false));expect(api.rescan).toHaveBeenCalledWith("legacy",{nameSuffix:null,legacy:{selectionRef:"exact-selection",durationSeconds:6,width:1280,height:768}});
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
 it("shows 构图用途 of each row in the preview",async()=>{
  const p=preview("usage",false);p.rows[1]={...p.rows[1],entityNames:["取账前过渡","取账尾"],openingComposition:"取账前过渡",endingComposition:"取账尾"};api.pick.mockResolvedValue(p);
  render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()}/>);await pick();
  expect(screen.getByText("开场构图：取账前过渡")).toBeTruthy();expect(screen.getByText("结尾构图：取账尾")).toBeTruthy();
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
  await act(async()=>scan.resolve({...preview("session",false),previewVersion:2,nameSuffix:" v2"}));expect(screen.getByRole("checkbox",{name:"导入 镜头2"})).toHaveProperty("disabled",false);expect(screen.getByRole("checkbox",{name:"导入 镜头2"})).toHaveProperty("checked",false);fireEvent.click(screen.getByRole("checkbox",{name:"导入 镜头2"}));fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入完成");expect(api.commit.mock.calls[0][0]).toMatchObject({previewVersion:2,nameSuffix:" v2",includeRowKeys:["r0","r1","r2"]});
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

it("offers each committed Shot as an independent edit target without restarting the package", async () => {
  const openShot=vi.fn(); const queue=vi.fn();
  render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()} onOpenQueue={queue} onOpenShot={openShot}/>);
  await pick();fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入完成");
  fireEvent.click(screen.getByRole("button",{name:"编辑「镜头1」"}));
  expect(openShot).toHaveBeenCalledExactlyOnceWith("s2");expect(queue).not.toHaveBeenCalled();
  expect(api.commit).toHaveBeenCalledTimes(1);expect(api.commit.mock.calls[0][0].autoStart).toBe(false);
});

it("does not offer per-Shot controls for rolled-back imports", async () => {
  api.commit.mockResolvedValue(result("compensated"));
  render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()} onOpenShot={vi.fn()}/>);
  await pick();fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入失败，本次新增的数据已撤回");
  expect(screen.queryByRole("region",{name:"逐镜控制"})).toBeNull();expect(screen.queryByRole("button",{name:/编辑「/})).toBeNull();
});

it("bounds committed Shot controls to 50 and preserves exact IDs when paging", async () => {
  const p=preview("large",false);p.rows=Array.from({length:101},(_,i)=>({...p.rows[1],rowKey:`r${i}`,rowNumber:i+1,name:`镜头${i}`}));p.shotCount=101;p.validCount=101;
  api.pick.mockResolvedValue(p);api.commit.mockResolvedValue({...result(),createdRows:p.rows.map((row,i)=>({rowKey:row.rowKey,shotId:`shot-${i}`,name:row.name})),createdShotIds:Array.from({length:101},(_,i)=>`shot-${i}`),batchId:null,itemCount:0});
  const openShot=vi.fn();render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()} onOpenShot={openShot}/>);
  await pick();fireEvent.click(screen.getByRole("checkbox",{name:"创建待启动批次"}));fireEvent.click(screen.getByRole("button",{name:"确认导入"}));await screen.findByText("导入完成");
  await waitFor(() => expect(screen.getByRole("region",{name:"文件夹批量导入"}).getAttribute("aria-busy")).toBe("false"));
  expect(screen.getAllByRole("button",{name:/编辑「/})).toHaveLength(50);fireEvent.click(screen.getByRole("button",{name:"下一组镜头"}));
  await waitFor(() => expect(screen.getByRole("button",{name:"编辑「镜头50」"})).toBeTruthy());
  fireEvent.click(screen.getByRole("button",{name:"编辑「镜头50」"}));expect(openShot).toHaveBeenCalledExactlyOnceWith("shot-50");
  expect(screen.getAllByRole("button",{name:/编辑「/})).toHaveLength(50);
});

it('AUDIT-F13 rescan of identical valid rows must retain explicit exclusions and import-only intent',async()=>{
 api.rescan.mockResolvedValue({...preview(),previewVersion:2,nameSuffix:'-new'});
 render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()}/>);await pick();fireEvent.click(screen.getByRole("checkbox",{name:"导入 镜头1"}));fireEvent.click(screen.getByRole("checkbox",{name:"创建待启动批次"}));
 fireEvent.change(screen.getByLabelText('镜头名称后缀'),{target:{value:'-new'}});fireEvent.click(screen.getByRole('button',{name:'重新校验'}));
 await waitFor(()=>expect(screen.getByRole('button',{name:'确认导入'})).toHaveProperty('disabled',false));
 const observed={excludedRowChecked:(screen.getByRole('checkbox',{name:'导入 镜头1'}) as HTMLInputElement).checked,queueChecked:(screen.getByRole('checkbox',{name:'创建待启动批次'}) as HTMLInputElement).checked};
 fireEvent.click(screen.getByRole('button',{name:'确认导入'}));await screen.findByText('导入完成');console.log('F13 '+JSON.stringify({...observed,request:api.commit.mock.calls[0][0]}));
 expect(api.commit.mock.calls[0][0]).toMatchObject({includeRowKeys:['r0'],createQueue:false,autoStart:false});
});
it('CONTROL without rescan import preserves explicit exclusions and does not prepare Queue',async()=>{
 render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()}/>);await pick();fireEvent.click(screen.getByRole("checkbox",{name:"导入 镜头1"}));fireEvent.click(screen.getByRole("checkbox",{name:"创建待启动批次"}));fireEvent.click(screen.getByRole('button',{name:'确认导入'}));await screen.findByText('导入完成');expect(api.commit.mock.calls[0][0]).toMatchObject({includeRowKeys:['r0'],createQueue:false,autoStart:false});
});

it("R66/R69: new passing rows are offered explicitly without restoring a user's exclusion, and range shortcuts are bounded",async()=>{
 api.rescan.mockResolvedValue({...preview("session",false),previewVersion:2});
 render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()}/>);await pick();
 fireEvent.click(screen.getByRole("checkbox",{name:"导入 镜头1"}));
 fireEvent.click(screen.getByRole("button",{name:"重新校验"}));
 const apply=await screen.findByRole("button",{name:"勾选本次新通过镜头"});
 expect(screen.getByRole("checkbox",{name:"导入 镜头1"})).toHaveProperty("checked",false);
 expect(screen.getByRole("checkbox",{name:"导入 镜头2"})).toHaveProperty("checked",false);
 fireEvent.click(apply);expect(screen.getByRole("checkbox",{name:"导入 镜头2"})).toHaveProperty("checked",true);
 expect(screen.getByRole("checkbox",{name:"导入 镜头1"})).toHaveProperty("checked",false);
 fireEvent.click(screen.getByRole("button",{name:"清空全部选择"}));expect(screen.getByRole("button",{name:"确认导入"})).toHaveProperty("disabled",true);
 fireEvent.click(screen.getByRole("button",{name:"仅选前 100 个可导入镜头"}));expect(screen.getByRole("checkbox",{name:"导入 镜头1"})).toHaveProperty("checked",true);
});
it("R49/R70: full prompt and Picture identities are inspectable and receipts never relabel IDs from current selection",async()=>{
 const p=preview();p.rows[0].prompt="完整正文末尾不可截断";p.rows[0].referencePreview=["z.png","a.png"];api.pick.mockResolvedValue(p);
 const receipt=result();receipt.createdRows=[{rowKey:"old",shotId:"s1",name:"原回执镜头名"}];receipt.createdShotIds=["s1"];api.commit.mockResolvedValue(receipt);
 render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()} onOpenShot={vi.fn()}/>);await pick();
 expect(screen.getByText("完整正文末尾不可截断")).toBeTruthy();expect(screen.getByText("Picture 1 · z.png")).toBeTruthy();expect(screen.getByText("Picture 2 · a.png")).toBeTruthy();
 fireEvent.click(screen.getByRole("checkbox",{name:"导入 镜头0"}));fireEvent.click(screen.getByRole("button",{name:"确认导入"}));
 expect(await screen.findByRole("button",{name:"编辑「原回执镜头名」"})).toBeTruthy();
});

it("filters preview rows by errors/selected without altering the committed includeRowKeys", async () => {
  render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()} />);
  await pick();
  fireEvent.click(screen.getByRole("button", { name: "仅看报错（1）" }));
  expect(screen.queryByRole("checkbox", { name: "导入 镜头0" })).toBeNull();
  expect(screen.getByRole("checkbox", { name: "导入 镜头2" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "确认导入" }));
  await screen.findByText("导入完成");
  expect(api.commit).toHaveBeenCalledWith(expect.objectContaining({ includeRowKeys: ["r0", "r1"] }));
});

it("returns to a valid selected-only page after removing the final row on page two", async () => {
  const seed = preview("many", false);
  const rows = Array.from({ length: 51 }, (_, i) => ({ ...seed.rows[0], rowKey: `r${i}`, rowNumber: i + 1, name: `镜头${i}` }));
  api.pick.mockResolvedValue({ ...seed, rows, shotCount: 51, validCount: 51 });
  render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()} />);
  await pick();
  fireEvent.click(screen.getByRole("button", { name: "仅看已选（51）" }));
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "导入 镜头50" }));
  expect(screen.getByRole("button", { name: "仅看已选（50）" })).toBeTruthy();
  expect(screen.getAllByRole("checkbox", { name: /导入 镜头/ })).toHaveLength(50);
  expect(screen.queryByRole("navigation", { name: "导入预览分页" })).toBeNull();
});

it("broadcasts entity, run, and video input invalidations on completed folder import commit", async () => {
  const onRun = vi.fn();
  const onInput = vi.fn();
  const onEntity = vi.fn();
  const unsubRun = subscribeRunInvalidation(onRun);
  const unsubInput = subscribeVideoInputInvalidation(onInput);
  window.addEventListener(STORY_ENTITIES_CHANGED, onEntity);
  try {
    api.commit.mockResolvedValueOnce({ ...result("completed"), createdEntityIds: ["ent-1"], importedAssetIds: ["ast-1"], batchId: "batch-1" });
    const view = render(<FolderBatchImportPanel projectId="p" onImported={vi.fn()} onCancel={vi.fn()} />);
    await pick();
    fireEvent.click(screen.getByRole("button", { name: "确认导入" }));
    await screen.findByText("导入完成");
    expect(onEntity).toHaveBeenCalledTimes(1);
    expect(onInput).toHaveBeenCalledWith("p");
    expect(onRun).toHaveBeenCalledWith("p", { library: true });

    onEntity.mockClear(); onInput.mockClear(); onRun.mockClear();
    api.commit.mockResolvedValueOnce(result("compensated"));
    view.rerender(<FolderBatchImportPanel projectId="p2" onImported={vi.fn()} onCancel={vi.fn()} />);
    await pick();
    fireEvent.click(screen.getByRole("button", { name: "确认导入" }));
    await screen.findByText("导入失败，本次新增的数据已撤回");
    expect(onEntity).not.toHaveBeenCalled();
    expect(onInput).not.toHaveBeenCalled();
    expect(onRun).not.toHaveBeenCalled();
  } finally {
    unsubRun();
    unsubInput();
    window.removeEventListener(STORY_ENTITIES_CHANGED, onEntity);
  }
});
