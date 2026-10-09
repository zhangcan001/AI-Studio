// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CreatePage } from "./CreatePage";
import { CreateMediaPreview } from "./CreateMediaPreview";
import { StrictMode } from "react";
import { useStudioStore } from "../../stores/studioStore";
import type { CreationAsset, CreationContext, GeneratorOption, ProductRun } from "../../product/types";
import type { VideoInputView } from "../../types/shotVideoInput";
const api = vi.hoisted(() => ({ get:vi.fn(),generatorsList:vi.fn(),inputsGet:vi.fn(),inputsSave:vi.fn(),assetsImport:vi.fn(),readinessGet:vi.fn(),generate:vi.fn(),selectResult:vi.fn(),runGet:vi.fn(),retry:vi.fn(),libraryGet:vi.fn(),imageGet:vi.fn(),mediaUrl:vi.fn() }));
vi.mock("../../product/client",()=>({productClient:{creation:api,run:{get:api.runGet,retry:api.retry},library:{get:api.libraryGet,imageGet:api.imageGet}}}));
const route = {kind:"create",projectId:"owned",shotId:"one",stage:"video"} as const;
const runtime = {status:"CONNECTED",endpoint:"http://fixture.invalid",devices:[],runtimeGeneration:1} as const;
const prompt = {type:"textarea",key:"prompt",label:"Prompt",required:true,default:""} as const;
const options:GeneratorOption[] = [
  {mode:"T2V",fields:[prompt]},
  {mode:"I2V",fields:[prompt,{type:"image",key:"first_frame",label:"First",required:true}]},
  {mode:"FIRST_LAST",fields:[prompt,{type:"image",key:"first_frame",label:"First",required:true},{type:"image",key:"last_frame",label:"Last",required:true}]},
  {mode:"REF2VA",fields:[prompt,{type:"images",key:"reference_images",label:"Images",required:false,minItems:0,maxItems:9},{type:"videos",key:"reference_videos",label:"Videos",required:false,minItems:0,maxItems:3},{type:"audios",key:"reference_audios",label:"Audios",required:false,minItems:0,maxItems:3}]},
].map((o,i)=>({...o,selectionRef:`opaque-${i}`,name:"Same display name",version:"2.2",mediaKind:"video",availability:true,availabilityReason:null,recommended:i===0,persistentInputs:true})) as GeneratorOption[];
const assets = [{id:"first",name:"外部首帧",mediaKind:"image"},{id:"last",name:"外部尾帧",mediaKind:"image"},{id:"ref-video",name:"外部视频",mediaKind:"video"},{id:"ref-video2",name:"视频二",mediaKind:"video"},{id:"ref-audio",name:"外部音频",mediaKind:"audio"}] satisfies VideoInputView["assets"];
let candidates:CreationAsset[], saved:Map<string,VideoInputView>;
const nav = vi.fn();
function context(projectId=route.projectId,shotId:string|null="one"):CreationContext { return {projectId,projectName:"Owned fixture",stage:"video",shots:[{id:"one",name:"镜头一",ordinal:0},{id:"two",name:"镜头二",ordinal:1}],selectedShot:shotId?{summary:{id:shotId,name:shotId,ordinal:0},prompt:"move",selectionRef:null,values:{},referenceAssetIds:[],selectedResultId:null,recentRun:null}:null,candidates,mediaInputs:[{id:"historical-image",name:"历史生成图",mediaKind:"image",selected:false}],promptChoices:[]}; }
function view(selectionRef:string):VideoInputView { return saved.get(selectionRef)??{selectionRef,token:null,inputs:[],assets}; }
function run(status:ProductRun["status"]="QUEUED",actions:string[]=[]):ProductRun { return {ref:{source:"queue-batch",id:"frozen"},projectId:"owned",title:"owned run",status,phase:status,createdAt:"",updatedAt:"",progress:{total:2,succeeded:0,failed:status==="FAILED"?2:0,cancelled:0},recoverability:{retryItemIds:["original"],reviewRequired:0},resultsSummary:[],errorSummary:null,preferredParent:null,availableActions:actions}; }
beforeEach(()=>{
  vi.resetAllMocks();useStudioStore.getState().resetDraft();candidates=[];saved=new Map();
  vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(()=>{});vi.spyOn(HTMLMediaElement.prototype,"load").mockImplementation(()=>{});
  api.get.mockImplementation((p,s)=>Promise.resolve(context(p,s)));api.generatorsList.mockResolvedValue(options);
  api.inputsGet.mockImplementation(s=>Promise.resolve(view(s.selectionRef)));
  api.inputsSave.mockImplementation(async r=>{const next={...view(r.selection.selectionRef),token:{instanceId:"receipt",revision:1},inputs:r.inputs};saved.set(r.selection.selectionRef,next);return next;});
  api.assetsImport.mockResolvedValue({imported:[],failed:[],cancelled:false});api.readinessGet.mockResolvedValue({ready:true,issues:[],fieldErrors:[],actions:[]});
  api.generate.mockResolvedValue({accepted:true,runRef:run().ref,startOutcome:"STARTED",startIssue:null});api.runGet.mockResolvedValue(run());
  api.selectResult.mockImplementation(async(_p,_s,_stage,id)=>{candidates=candidates.map(a=>({...a,selected:a.id===id}));});
  api.mediaUrl.mockImplementation((p,id)=>`http://fixture.invalid/${p}/${id}`);
  api.libraryGet.mockResolvedValue({kind:"asset",asset:{assetType:"image",mimeType:"image/png"}});api.imageGet.mockResolvedValue([1]);
  vi.stubGlobal("URL",{createObjectURL:vi.fn(()=>"blob:owned"),revokeObjectURL:vi.fn()});
});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
async function start() { const result=render(<CreatePage route={route} runtime={{...runtime,devices:[]}} navigate={nav}/>);await screen.findByLabelText("选择生成器");await waitFor(()=>expect(screen.getByRole("button",{name:"生成"}).hasAttribute("disabled")).toBe(false));return result; }
async function mode(ref:string) {fireEvent.change(screen.getByLabelText("选择生成器"),{target:{value:ref}});await waitFor(()=>expect(api.inputsGet).toHaveBeenLastCalledWith({projectId:"owned",shotId:"one",selectionRef:ref}));await waitFor(()=>expect(screen.getByRole("button",{name:"导入图片、视频或音频"}).hasAttribute("disabled")).toBe(false));}
async function save() {fireEvent.click(screen.getByRole("button",{name:"保存视频输入"}));await waitFor(()=>expect(screen.queryByText("输入尚未保存。")).toBeNull());}
it("T2V has no media requirement, uses submit-time authority and prevents double submission",async()=>{
  await start();expect(screen.getByText("T2V 无需图片输入。")).toBeTruthy();expect(screen.getByLabelText("视频预览")).toBeTruthy();expect(screen.queryByRole("video")).toBeNull();
  let finish!:(v:unknown)=>void;api.readinessGet.mockImplementationOnce(()=>new Promise(r=>{finish=r;}));fireEvent.click(screen.getByRole("button",{name:"生成"}));fireEvent.click(screen.getByRole("button",{name:"正在提交…"}));
  await act(async()=>finish({ready:true,issues:[],fieldErrors:[],actions:[]}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));expect(api.generate.mock.calls[0][0].values.first_frame).toBeUndefined();expect(await screen.findByText("请求已接受")).toBeTruthy();expect(screen.queryByText("已完成")).toBeNull();
});
it("I2V missing-first is rejected by shared readiness; imported input is not historical generated image",async()=>{
  await start();await mode("opaque-1");expect(screen.queryByRole("option",{name:"历史生成图"})).toBeNull();
  api.readinessGet.mockResolvedValue({ready:false,issues:[{code:"INPUT_REQUIRED",details:{field:"first_frame",action:"EDIT_INPUT"}}],fieldErrors:[],actions:[]});fireEvent.click(screen.getByRole("button",{name:"生成"}));await waitFor(()=>expect(api.readinessGet).toHaveBeenCalled());expect(api.generate).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("首帧"),{target:{value:"first"}});expect(useStudioStore.getState().values.first_frame).toEqual({type:"image_asset",assetId:"first"});await save();expect(api.selectResult).not.toHaveBeenCalled();
});
it("FIRST_LAST stays independent and switching Recipe restores only its own saved slots",async()=>{
  await start();await mode("opaque-2");fireEvent.change(screen.getByLabelText("首帧"),{target:{value:"first"}});expect(useStudioStore.getState().values.last_frame).toBeUndefined();fireEvent.change(screen.getByLabelText("尾帧"),{target:{value:"last"}});await save();
  expect(api.inputsSave.mock.calls[0][0].inputs).toEqual([{inputKey:"first_frame",ordinal:0,assetId:"first"},{inputKey:"last_frame",ordinal:0,assetId:"last"}]);
  await mode("opaque-1");expect(useStudioStore.getState().values.first_frame).toBeUndefined();await mode("opaque-2");await waitFor(()=>expect(useStudioStore.getState().values.last_frame).toEqual({type:"image_asset",assetId:"last"}));
});
it("REF2VA hands ordered mixed media, video-only and pure-audio cases to the single authority",async()=>{
  await start();await mode("opaque-3");
  fireEvent.change(screen.getByLabelText("参考视频"),{target:{value:"ref-video"}});fireEvent.change(screen.getByLabelText("参考视频"),{target:{value:"ref-video2"}});fireEvent.click(screen.getByRole("button",{name:"参考视频下移第1项"}));await save();
  fireEvent.click(screen.getByRole("button",{name:"生成"}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));expect(api.generate.mock.calls[0][0].values.reference_videos).toEqual({type:"video_assets",assetIds:["ref-video2","ref-video"]});expect(api.generate.mock.calls[0][0].values.reference_images).toBeUndefined();
  fireEvent.change(screen.getByLabelText("参考图片"),{target:{value:"first"}});fireEvent.change(screen.getByLabelText("参考音频"),{target:{value:"ref-audio"}});await save();fireEvent.click(screen.getByRole("button",{name:"生成"}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));expect(api.generate.mock.calls[1][0].values.reference_audios).toEqual({type:"audio_assets",assetIds:["ref-audio"]});
  fireEvent.click(screen.getByRole("button",{name:"参考图片移除第1项"}));fireEvent.click(screen.getByRole("button",{name:"参考视频移除第1项"}));fireEvent.click(screen.getByRole("button",{name:"参考视频移除第1项"}));await save();
  api.readinessGet.mockResolvedValue({ready:false,issues:[],fieldErrors:[],actions:[]});fireEvent.click(screen.getByRole("button",{name:"生成"}));await waitFor(()=>expect(api.readinessGet.mock.calls[api.readinessGet.mock.calls.length-1]?.[0].values.reference_videos).toBeUndefined());expect(api.generate).toHaveBeenCalledTimes(2);
});
it("one player, preview is not selection, explicit selection is not review, missing asset is honest",async()=>{
  candidates=[{id:"a",name:"候选A",mediaKind:"video",selected:false},{id:"b",name:"候选B",mediaKind:"video",selected:false}];const page=await start();
  expect(document.querySelectorAll("video")).toHaveLength(1);fireEvent.click(screen.getByRole("button",{name:"预览候选B"}));expect(api.selectResult).not.toHaveBeenCalled();expect(document.querySelector("video")?.getAttribute("src")).toContain("owned/b");
  fireEvent.click(screen.getByRole("button",{name:"选用此结果"}));await waitFor(()=>expect(api.selectResult).toHaveBeenCalledWith("owned","one","video","b"));expect(screen.getByText(/切换仅预览/)).toBeTruthy();expect(screen.getByRole("button",{name:"预览候选A"})).toBeTruthy();
  fireEvent.error(document.querySelector("video")!);expect(await screen.findByText(/媒体不可用或无法预览/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"预览候选A"}));fireEvent.error(document.querySelector("video")!);fireEvent.click(screen.getByRole("button",{name:"预览候选B"}));expect(screen.getByRole("button",{name:"选用此结果"}).hasAttribute("disabled")).toBe(true);
  page.rerender(<CreatePage route={{...route,projectId:"other",shotId:"two"}} runtime={{...runtime,devices:[]}} navigate={nav}/>);await screen.findByLabelText("选择生成器");await waitFor(()=>expect(api.mediaUrl).toHaveBeenLastCalledWith("other","b","video"));expect(HTMLMediaElement.prototype.pause).toHaveBeenCalled();expect(HTMLMediaElement.prototype.load).toHaveBeenCalled();
});
it("failed-to-start is not success, retry uses frozen RunRef despite draft edits",async()=>{
  api.generate.mockResolvedValue({accepted:true,runRef:run().ref,startOutcome:"FAILED_TO_START",startIssue:null});api.runGet.mockResolvedValue(run("FAILED",["RETRY"]));api.retry.mockResolvedValue(run());await start();fireEvent.click(screen.getByRole("button",{name:"生成"}));expect(await screen.findByText("已加入队列，启动失败")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("提示词"),{target:{value:"edited draft"}});fireEvent.click(await screen.findByRole("button",{name:"重试原运行"}));await waitFor(()=>expect(api.retry).toHaveBeenCalledWith("owned",{ref:run().ref,selectedItemIds:["original"]}));expect(api.generate).toHaveBeenCalledTimes(1);fireEvent.click(screen.getByRole("button",{name:"查看运行详情"}));expect(nav).toHaveBeenLastCalledWith({kind:"runs",projectId:"owned",run:run().ref});
});
it("image previews ignore late owner reads, revoke URLs, and video detaches on unmount",async()=>{
  let finish!:(bytes:number[])=>void;api.imageGet.mockImplementationOnce(()=>new Promise(r=>{finish=r;}));const asset={id:"first",name:"owned image",mediaKind:"image"} as const;
  const page=render(<CreateMediaPreview projectId="owned" asset={asset}/>);await waitFor(()=>expect(api.imageGet).toHaveBeenCalled());page.rerender(<CreateMediaPreview key="other" projectId="other" asset={{...asset,id:"last"}}/>);await screen.findByRole("img");await act(async()=>finish([2]));expect(URL.createObjectURL).toHaveBeenCalledTimes(1);page.unmount();expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:owned");
  const player=render(<CreateMediaPreview projectId="owned" asset={{id:"v",name:"video",mediaKind:"video"}}/>);const node=document.querySelector("video")!;player.unmount();expect(node.getAttribute("src")).toBeNull();expect(node.autoplay).toBe(false);
});
it("StrictMode replay restores the controlled stream after its cleanup without autoplay",()=>{
  const page=render(<StrictMode><CreateMediaPreview projectId="owned" asset={{id:"v",name:"strict video",mediaKind:"video"}}/></StrictMode>);
  const node=document.querySelector("video")!;expect(node.getAttribute("src")).toBe("http://fixture.invalid/owned/v");expect(node.autoplay).toBe(false);expect(HTMLMediaElement.prototype.load).toHaveBeenCalled();page.unmount();expect(node.getAttribute("src")).toBeNull();
});
it("Library return preserves unsaved persistent media only for the exact owner",async()=>{
  const page=await start();await mode("opaque-1");fireEvent.change(screen.getByLabelText("首帧"),{target:{value:"first"}});
  fireEvent.click(screen.getByRole("button",{name:"在资源库查找更多提示词"}));expect(nav).toHaveBeenLastCalledWith({kind:"library",projectId:"owned",filter:"prompts"});page.unmount();
  const returned=render(<CreatePage route={route} runtime={{...runtime,devices:[]}} navigate={nav}/>);await screen.findByLabelText("选择生成器");await waitFor(()=>expect(useStudioStore.getState().values.first_frame).toEqual({type:"image_asset",assetId:"first"}));expect(await screen.findByText("输入尚未保存。")).toBeTruthy();expect(api.inputsSave).not.toHaveBeenCalled();expect(screen.getByRole("button",{name:"生成"}).hasAttribute("disabled")).toBe(true);
  returned.rerender(<CreatePage route={{...route,projectId:"other",shotId:"two"}} runtime={{...runtime,devices:[]}} navigate={nav}/>);await waitFor(()=>expect(useStudioStore.getState().values.first_frame).toBeUndefined());
});
