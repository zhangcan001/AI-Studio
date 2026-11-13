// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CreatePage } from "./CreatePage";
import { CreateMediaPreview } from "./CreateMediaPreview";
import { queueRunLabel } from "./createModel";
import { StrictMode } from "react";
import { useStudioStore } from "../../stores/studioStore";
import type { CreationAsset, CreationContext, GeneratorOption, ProductRun } from "../../product/types";
import type { VideoInputView } from "../../types/shotVideoInput";
import { setPrompt } from "./composerTestUtils";
const api = vi.hoisted(() => ({ get:vi.fn(),generatorsList:vi.fn(),inputsGet:vi.fn(),inputsSave:vi.fn(),assetsImport:vi.fn(),readinessGet:vi.fn(),generate:vi.fn(),selectResult:vi.fn(),runGet:vi.fn(),retry:vi.fn(),libraryGet:vi.fn(),imageGet:vi.fn(),mediaUrl:vi.fn() }));
vi.mock("../../product/client",()=>({productClient:{storyEntities:{list:vi.fn(async()=>[]),listForShot:vi.fn(async()=>[]),bindShot:vi.fn(async()=>[]),create:vi.fn(),update:vi.fn(),delete:vi.fn(),get:vi.fn()},creation:api,run:{get:api.runGet,retry:api.retry},library:{get:api.libraryGet,imageGet:api.imageGet,list:vi.fn(async()=>({items:[],nextCursor:null})),tagsList:vi.fn(async()=>[])}}}));
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
function context(projectId=route.projectId,shotId:string|null="one"):CreationContext { return {projectId,projectName:"Owned fixture",stage:"video",shots:[{id:"one",name:"镜头一",ordinal:0},{id:"two",name:"镜头二",ordinal:1}],selectedShot:shotId?{summary:{id:shotId,name:shotId,ordinal:0},prompt:"move",selectionRef:"opaque-0",values:{},referenceAssetIds:[],selectedResultId:null,recentRun:null}:null,candidates,mediaInputs:[{id:"historical-image",name:"历史生成图",mediaKind:"image",selected:false}],promptChoices:[]}; }
function view(selectionRef:string):VideoInputView { return saved.get(selectionRef)??{selectionRef,token:null,inputs:[],assets}; }
function run(status:ProductRun["status"]="QUEUED",actions:string[]=[]):ProductRun { return {ref:{source:"queue-batch",id:"frozen"},projectId:"owned",title:"owned run",status,phase:status,createdAt:"",updatedAt:"",progress:{total:2,succeeded:0,failed:status==="FAILED"?2:0,cancelled:0},recoverability:{retryItemIds:["original"],reviewRequired:0},resultsSummary:[],errorSummary:null,preferredParent:null,availableActions:actions}; }
beforeEach(()=>{
  vi.resetAllMocks();useStudioStore.getState().resetDraft();candidates=[];saved=new Map();sessionStorage.clear();
  vi.spyOn(window,"confirm").mockReturnValue(true);
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
afterEach(()=>{vi.useRealTimers();cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();sessionStorage.clear();});
async function start() { const result=render(<CreatePage route={route} runtime={{...runtime,devices:[]}} navigate={nav}/>);await screen.findByLabelText("选择生成器");await waitFor(()=>expect(screen.getByRole("button",{name:"生成"}).hasAttribute("disabled")).toBe(false));return result; }
async function mode(ref:string) {fireEvent.change(screen.getByLabelText("选择生成器"),{target:{value:ref}});await waitFor(()=>expect(api.inputsGet).toHaveBeenLastCalledWith({projectId:"owned",shotId:"one",selectionRef:ref}));await waitFor(()=>expect(composerInputs()).toBe("saved"));}
function composerInputs() { return screen.getByRole("region",{name:"创作输入框"}).getAttribute("data-inputs"); }
/** Media edits autosave through the existing OCC inputsSave; wait until the composer reports the draft persisted. */
async function save() {await waitFor(()=>expect(api.inputsSave).toHaveBeenCalled());await waitFor(()=>expect(composerInputs()).toBe("saved"));}
async function readyToGenerate() {await screen.findByText("可以入队生成");await waitFor(()=>expect(screen.getByRole("button",{name:"生成"}).hasAttribute("disabled")).toBe(false));}
async function pickImage(label:string, name:string) {
  fireEvent.click(screen.getByLabelText(label));
  fireEvent.click(within(await screen.findByRole("dialog")).getByRole("option",{name:new RegExp(name)}));
}
function clearImage(label:string) { fireEvent.click(screen.getByRole("button",{name:`${label}移除第1项`})); }

it("T2V has no media requirement, uses submit-time authority and prevents double submission",async()=>{
  await start();expect(screen.getByText("T2V 无需图片输入。")).toBeTruthy();expect(screen.getByLabelText("视频预览")).toBeTruthy();expect(screen.queryByRole("video")).toBeNull();
  let finish!:(v:unknown)=>void;api.readinessGet.mockImplementationOnce(()=>new Promise(r=>{finish=r;}));fireEvent.click(screen.getByRole("button",{name:"生成"}));fireEvent.click(screen.getByRole("button",{name:"正在提交…"}));
  await act(async()=>finish({ready:true,issues:[],fieldErrors:[],actions:[]}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));expect(api.generate.mock.calls[0][0].values.first_frame).toBeUndefined();expect(await screen.findByText("请求已接受")).toBeTruthy();expect(screen.queryByText("已完成")).toBeNull();
});
it("I2V missing-first is rejected by shared readiness; imported input is not historical generated image",async()=>{
  await start();await mode("opaque-1");expect(screen.queryByRole("option",{name:"历史生成图"})).toBeNull();
  api.readinessGet.mockResolvedValue({ready:false,issues:[{code:"INPUT_REQUIRED",details:{field:"first_frame",action:"EDIT_INPUT"}}],fieldErrors:[],actions:[]});fireEvent.click(screen.getByRole("button",{name:"生成"}));await waitFor(()=>expect(api.readinessGet).toHaveBeenCalled());expect(api.generate).not.toHaveBeenCalled();
  await pickImage("首帧","外部首帧");expect(useStudioStore.getState().values.first_frame).toEqual({type:"image_asset",assetId:"first"});await save();expect(api.selectResult).not.toHaveBeenCalled();
});
it("FIRST_LAST stays independent and switching Recipe restores only its own saved slots",async()=>{
  await start();await mode("opaque-2");await pickImage("首帧","外部首帧");expect(useStudioStore.getState().values.last_frame).toBeUndefined();await pickImage("尾帧","外部尾帧");await save();
  expect(api.inputsSave.mock.calls[0][0].inputs).toEqual([{inputKey:"first_frame",ordinal:0,assetId:"first"},{inputKey:"last_frame",ordinal:0,assetId:"last"}]);
  await mode("opaque-1");expect(useStudioStore.getState().values.first_frame).toBeUndefined();await mode("opaque-2");await waitFor(()=>expect(useStudioStore.getState().values.last_frame).toEqual({type:"image_asset",assetId:"last"}));
});
it("REF2VA hands ordered mixed media, video-only and pure-audio cases to the single authority",async()=>{
  await start();await mode("opaque-3");
  fireEvent.change(screen.getByLabelText("参考视频"),{target:{value:"ref-video"}});fireEvent.change(screen.getByLabelText("参考视频"),{target:{value:"ref-video2"}});fireEvent.click(screen.getByRole("button",{name:"参考视频下移第1项"}));await save();
  await readyToGenerate();fireEvent.click(screen.getByRole("button",{name:"生成"}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));expect(api.generate.mock.calls[0][0].values.reference_videos).toEqual({type:"video_assets",assetIds:["ref-video2","ref-video"]});expect(api.generate.mock.calls[0][0].values.reference_images).toBeUndefined();
  await pickImage("参考图片","外部首帧");fireEvent.change(screen.getByLabelText("参考音频"),{target:{value:"ref-audio"}});await save();await readyToGenerate();fireEvent.click(screen.getByRole("button",{name:"生成"}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));expect(api.generate.mock.calls[1][0].values.reference_audios).toEqual({type:"audio_assets",assetIds:["ref-audio"]});
  // Readiness is debounced (600ms); drive that timer explicitly instead of racing waitFor's 1s budget.
  vi.useFakeTimers({shouldAdvanceTime:true});
  api.readinessGet.mockResolvedValue({ready:false,issues:[],fieldErrors:[],actions:[]});
  fireEvent.click(screen.getByRole("button",{name:"参考图片移除第1项"}));fireEvent.click(screen.getByRole("button",{name:"参考视频移除第1项"}));fireEvent.click(screen.getByRole("button",{name:"参考视频移除第1项"}));await save();
  await act(async()=>{await vi.advanceTimersByTimeAsync(600);});
  await waitFor(()=>expect(api.readinessGet.mock.calls[api.readinessGet.mock.calls.length-1]?.[0].values.reference_videos).toBeUndefined());
  vi.useRealTimers();
  await waitFor(()=>expect(screen.getByRole("button",{name:"生成"}).hasAttribute("disabled")).toBe(true));
  expect(api.generate).toHaveBeenCalledTimes(2);
});
it("one player, preview is not selection, explicit selection is not review, missing asset is honest",async()=>{
  candidates=[{id:"a",name:"候选A",mediaKind:"video",selected:false},{id:"b",name:"候选B",mediaKind:"video",selected:false}];const page=await start();
  expect(document.querySelectorAll("video:not(.create-candidate-frame)")).toHaveLength(1);fireEvent.click(screen.getByRole("button",{name:"预览候选B"}));expect(api.selectResult).not.toHaveBeenCalled();expect(document.querySelector("video:not(.create-candidate-frame)")?.getAttribute("src")).toContain("owned/b");
  fireEvent.click(screen.getByRole("button",{name:"选用此结果"}));await waitFor(()=>expect(api.selectResult).toHaveBeenCalledWith("owned","one","video","b"));expect(screen.getByText(/切换仅预览/)).toBeTruthy();expect(screen.getByRole("button",{name:"预览候选A"})).toBeTruthy();
  fireEvent.error(document.querySelector("video")!);expect(await screen.findByText(/媒体不可用或无法预览/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"预览候选A"}));fireEvent.error(document.querySelector("video")!);fireEvent.click(screen.getByRole("button",{name:"预览候选B"}));expect(screen.getByRole("button",{name:"选用此结果"}).hasAttribute("disabled")).toBe(true);
  page.rerender(<CreatePage route={{...route,projectId:"other",shotId:"two"}} runtime={{...runtime,devices:[]}} navigate={nav}/>);await screen.findByLabelText("选择生成器");await waitFor(()=>expect(api.mediaUrl).toHaveBeenLastCalledWith("other","b","video"));expect(HTMLMediaElement.prototype.pause).toHaveBeenCalled();expect(HTMLMediaElement.prototype.load).toHaveBeenCalled();
});
it("failed-to-start is not success, retry uses frozen RunRef despite draft edits",async()=>{
  api.generate.mockResolvedValue({accepted:true,runRef:run().ref,startOutcome:"FAILED_TO_START",startIssue:null});api.runGet.mockResolvedValue(run("FAILED",["RETRY"]));api.retry.mockResolvedValue(run());await start();fireEvent.click(screen.getByRole("button",{name:"生成"}));expect(await screen.findByText("已保存运行，启动失败")).toBeTruthy();
  setPrompt("edited draft");fireEvent.click(await screen.findByRole("button",{name:"重试失败项（原输入）"}));await waitFor(()=>expect(api.retry).toHaveBeenCalledWith("owned",{ref:run().ref,selectedItemIds:["original"]}));expect(api.generate).toHaveBeenCalledTimes(1);fireEvent.click(screen.getByRole("button",{name:"查看运行详情"}));expect(nav).toHaveBeenLastCalledWith({kind:"runs",projectId:"owned",run:run().ref});
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
it("clearing the Shot picker navigates to an unselected Shot without losing the picker",async()=>{
  const page=await start();
  fireEvent.change(screen.getByLabelText("镜头"),{target:{value:""}});
  expect(nav).toHaveBeenLastCalledWith({...route,shotId:undefined});
  api.get.mockImplementation((p,s)=>s===""?Promise.reject(new Error("invalid Shot")):Promise.resolve(context(p,s)));
  page.rerender(<CreatePage route={{...route,shotId:undefined}} runtime={{...runtime,devices:[]}} navigate={nav}/>);
  await waitFor(()=>expect(api.get).toHaveBeenLastCalledWith("owned",null,"video"));
  expect(await screen.findByText("选择一个镜头开始创作。")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("镜头"),{target:{value:"two"}});
  expect(nav).toHaveBeenLastCalledWith({...route,shotId:"two"});
});
it("a saved media OCC revision replaces stale missing-input readiness without draft edits",async()=>{
  api.inputsSave.mockImplementation(async r=>{
    const old=view(r.selection.selectionRef);
    const next={...old,token:{instanceId:"receipt",revision:(old.token?.revision??0)+1},inputs:r.inputs};
    saved.set(r.selection.selectionRef,next);return next;
  });
  await start();await mode("opaque-2");
  api.readinessGet.mockImplementation(async request=>{
    const bindings=view(request.selectionRef).inputs;
    const complete=bindings.some(i=>i.inputKey==="first_frame") && bindings.some(i=>i.inputKey==="last_frame");
    return {ready:complete,issues:complete?[]:[{code:"INPUT_REQUIRED",details:{field:"first_frame",action:"EDIT_INPUT"}}],fieldErrors:[],actions:[]};
  });
  await pickImage("首帧","外部首帧");
  await pickImage("尾帧","外部尾帧");
  await save();await waitFor(()=>expect(screen.getByText("可以入队生成")).toBeTruthy());
  clearImage("首帧");
  await save();await waitFor(()=>expect(screen.getByText("请检查输入或运行环境")).toBeTruthy());
  const before=api.readinessGet.mock.calls.length;
  await pickImage("首帧","外部首帧");
  await save();
  await waitFor(()=>expect(api.readinessGet.mock.calls.length).toBeGreaterThan(before));
  await waitFor(()=>expect(screen.getByText("可以入队生成")).toBeTruthy());
  expect(api.generate).not.toHaveBeenCalled();
});
it("Library return preserves unsaved persistent media only for the exact owner",async()=>{
  const page=await start();await mode("opaque-1");await pickImage("首帧","外部首帧");
  fireEvent.click(screen.getByRole("button",{name:"在资源库查找更多提示词"}));expect(nav).toHaveBeenLastCalledWith({kind:"library",projectId:"owned",filter:"prompts"});page.unmount();
  const returned=render(<CreatePage route={route} runtime={{...runtime,devices:[]}} navigate={nav}/>);await screen.findByLabelText("选择生成器");await waitFor(()=>expect(useStudioStore.getState().values.first_frame).toEqual({type:"image_asset",assetId:"first"}));expect(await screen.findByText("输入尚未保存。")).toBeTruthy();expect(api.inputsSave).not.toHaveBeenCalled();expect(screen.getByRole("button",{name:"生成"}).hasAttribute("disabled")).toBe(true);
  returned.rerender(<CreatePage route={{...route,projectId:"other",shotId:"two"}} runtime={{...runtime,devices:[]}} navigate={nav}/>);await waitFor(()=>expect(useStudioStore.getState().values.first_frame).toBeUndefined());
});
it("labels a saved run whose Start was rejected as 无法启动, never as queued", () => {
  expect(queueRunLabel({ status: "PAUSED", errorSummary: "排队任务无法自动启动：RUNTIME_ADMISSION_CAPABILITY_UNKNOWN" })).toBe("无法启动");
  expect(queueRunLabel({ status: "PAUSED", errorSummary: null })).toBe("暂停");
  expect(queueRunLabel({ status: "QUEUED", errorSummary: null })).toBe("排队中");
});

it("offers upgrade-to-final and Standard rerun actions without restoring fast preview", async () => {
  const presets = [
    { id: "mp-0_5-16x9", label: "0.5MP · 960×544", width: 960, height: 544 },
    { id: "mp-0_98-16x9", label: "0.98MP · 1344×768", width: 1344, height: 768 },
  ];
  const pika = { ...options[3], selectionRef: "opaque-pika", name: "MiniMax H3 Pika 全能参考 · 8步", recommended: true, resolutionPresets: presets };
  const standard = { ...pika, selectionRef: "opaque-std", name: "MiniMax H3 Standard 全能参考 · 20步", recommended: false };
  api.generatorsList.mockResolvedValue([options[0], options[1], options[2], pika, standard]);
  render(<CreatePage route={route} runtime={{...runtime,devices:[]}} navigate={nav}/>);
  await screen.findByLabelText("选择生成器");
  await waitFor(() => expect(screen.getByRole("button", { name: "生成" }).hasAttribute("disabled")).toBe(false));
  fireEvent.change(screen.getByLabelText("选择生成器"), { target: { value: "opaque-pika" } });
  await waitFor(() => expect(api.inputsGet).toHaveBeenLastCalledWith({ projectId: "owned", shotId: "one", selectionRef: "opaque-pika" }));
  await waitFor(() => expect(composerInputs()).toBe("saved"));
  await readyToGenerate();
  useStudioStore.getState().loadCreationDraft({ prompt: { type: "string", value: "test" }, width: { type: "integer", value: 960 }, height: { type: "integer", value: 544 } }, true);
  await readyToGenerate();
  await waitFor(() => expect(screen.getByRole("button", { name: "升成片分辨率再生成" }).hasAttribute("disabled")).toBe(false));
  expect(screen.getByRole("button", { name: "换 Standard 重跑" }).hasAttribute("disabled")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "升成片分辨率再生成" }));
  await waitFor(() => expect(api.generate).toHaveBeenCalled());
  const last = api.generate.mock.calls[api.generate.mock.calls.length - 1][0];
  expect(last.values.width).toEqual({ type: "integer", value: 1344 });
  expect(last.values.height).toEqual({ type: "integer", value: 768 });
  expect(last.selectionRef).toBe("opaque-pika");
});

it("a confirmed generation lets the next explicit unchanged generation use a new key", async () => {
  api.runGet.mockResolvedValue(run("SUCCEEDED"));
  await start();
  fireEvent.click(screen.getByRole("button", { name: "生成" }));
  await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(screen.getByRole("button", { name: "按当前设置再生成" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "按当前设置再生成" }));
  await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(2));
  expect(api.generate.mock.calls[1][0].values).toEqual(api.generate.mock.calls[0][0].values);
  expect(api.generate.mock.calls[1][0].submissionIdempotencyKey).not.toBe(api.generate.mock.calls[0][0].submissionIdempotencyKey);
});

it("an uncertain generation response retains the same key for a safe retry", async () => {
  api.generate.mockRejectedValueOnce(new Error("response lost"));
  await start();
  fireEvent.click(screen.getByRole("button", { name: "生成" }));
  await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(screen.getByRole("button", { name: "生成" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "生成" }));
  await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(2));
  expect(api.generate.mock.calls[1][0].submissionIdempotencyKey).toBe(api.generate.mock.calls[0][0].submissionIdempotencyKey);
});

function standardRerunFixture() {
  const pika = { ...options[3], selectionRef: "opaque-pika", name: "MiniMax H3 Pika 全能参考 · 8步", recommended: true };
  const standard = { ...pika, selectionRef: "opaque-std", name: "MiniMax H3 Standard 全能参考 · 20步", recommended: false };
  api.generatorsList.mockResolvedValue([pika, standard]);
  api.get.mockImplementation((p, s) => {
    const c = context(p, s);
    return Promise.resolve({ ...c, selectedShot: c.selectedShot ? { ...c.selectedShot, selectionRef: pika.selectionRef } : null });
  });
  saved.set(pika.selectionRef, { selectionRef: pika.selectionRef, token: { instanceId: "pika-inputs", revision: 1 }, inputs: [{ inputKey: "reference_images", ordinal: 0, assetId: "first" }], assets });
  saved.set(standard.selectionRef, { selectionRef: standard.selectionRef, token: { instanceId: "standard-inputs", revision: 3 }, inputs: [{ inputKey: "reference_images", ordinal: 0, assetId: "last" }], assets });
  return { pika, standard };
}

it("Standard rerun waits for its own saved Recipe inputs then submits once with fresh evidence", async () => {
  const { standard } = standardRerunFixture();
  api.readinessGet.mockImplementation(() => new Promise(resolve => setTimeout(() => resolve({ ready: true, issues: [], fieldErrors: [], actions: [] }), 30)));
  await start();
  fireEvent.click(screen.getByRole("button", { name: "换 Standard 重跑" }));
  await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(1));
  expect(api.generate).toHaveBeenCalledWith(expect.objectContaining({ selectionRef: standard.selectionRef, values: expect.objectContaining({ reference_images: { type: "image_assets", assetIds: ["last"] } }) }));
  expect(api.inputsSave).not.toHaveBeenCalled();
  await waitFor(() => expect(screen.getByRole("button", { name: "生成" }).hasAttribute("disabled")).toBe(false));
  expect(api.generate).toHaveBeenCalledTimes(1);
});

it("Standard rerun does not submit after changing Shot while its target inputs are in flight", async () => {
  const { standard } = standardRerunFixture();
  let release!: (value: VideoInputView) => void;
  api.inputsGet.mockImplementation(s => s.selectionRef === standard.selectionRef ? new Promise<VideoInputView>(resolve => { release = resolve; }) : Promise.resolve(view(s.selectionRef)));
  const page = await start();
  fireEvent.click(screen.getByRole("button", { name: "换 Standard 重跑" }));
  await waitFor(() => expect(release).toBeTypeOf("function"));
  page.rerender(<CreatePage route={{ ...route, shotId: "two" }} runtime={{ ...runtime, devices: [] }} navigate={nav} />);
  await waitFor(() => expect(api.get).toHaveBeenCalledWith("owned", "two", "video"));
  await act(async () => { release(view(standard.selectionRef)); });
  await readyToGenerate();
  expect(api.generate).not.toHaveBeenCalled();
});

it("a failed Standard input load releases the action without copying inputs or generating", async () => {
  const { standard } = standardRerunFixture();
  api.inputsGet.mockImplementation(s => s.selectionRef === standard.selectionRef ? Promise.reject(new Error("target inputs unavailable")) : Promise.resolve(view(s.selectionRef)));
  await start();
  fireEvent.click(screen.getByRole("button", { name: "换 Standard 重跑" }));
  await waitFor(() => expect(screen.getByLabelText<HTMLSelectElement>("选择生成器").value).toBe(standard.selectionRef));
  await waitFor(() => expect(screen.getByLabelText("包变体").hasAttribute("disabled")).toBe(false));
  expect(api.generate).not.toHaveBeenCalled();
  expect(api.inputsSave).not.toHaveBeenCalled();
  // Recipe defaults represent an unbound plural input as an empty asset list.
  expect(useStudioStore.getState().values.reference_images).toEqual({ type: "image_assets", assetIds: [] });
});

it("Standard rerun obeys a negative fresh preflight and does not automatically retry it", async () => {
  const { standard } = standardRerunFixture();
  await start();
  api.readinessGet.mockResolvedValue({ ready: false, issues: [], fieldErrors: [], actions: [] });
  fireEvent.click(screen.getByRole("button", { name: "换 Standard 重跑" }));
  await waitFor(() => expect(api.readinessGet).toHaveBeenCalledWith(expect.objectContaining({ selectionRef: standard.selectionRef })));
  await waitFor(() => expect(screen.getByLabelText("包变体").hasAttribute("disabled")).toBe(false));
  expect(api.generate).not.toHaveBeenCalled();
});

it('changed formal inputs after an uncertain response require a new intent key',async()=>{
 api.get.mockImplementation((p,s)=>{const c=context(p,s);return Promise.resolve({...c,selectedShot:c.selectedShot?{...c.selectedShot,selectionRef:'opaque-1'}:null});});
 saved.set('opaque-1',{selectionRef:'opaque-1',token:{instanceId:'i2v',revision:1},inputs:[{inputKey:'first_frame',ordinal:0,assetId:'first'}],assets});
 api.generate.mockRejectedValueOnce(new Error('ack lost'));
 await start();fireEvent.click(screen.getByRole('button',{name:'生成'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成'}).hasAttribute('disabled')).toBe(false));
 saved.set('opaque-1',{selectionRef:'opaque-1',token:{instanceId:'i2v',revision:2},inputs:[{inputKey:'first_frame',ordinal:0,assetId:'last'}],assets});
 fireEvent.click(screen.getByText('高级设置'));
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'刷新已保存输入'}));});
 await waitFor(()=>expect(useStudioStore.getState().values.first_frame).toEqual({type:'image_asset',assetId:'last'}));
 await readyToGenerate();fireEvent.click(screen.getByRole('button',{name:'生成'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
 const [a,b]=api.generate.mock.calls.map(call=>call[0]);
 expect(a.values.first_frame.assetId).toBe('first');expect(b.values.first_frame.assetId).toBe('last');
 expect(b.submissionIdempotencyKey).not.toBe(a.submissionIdempotencyKey);
});
it('uncertain retry after a semantically identical input refresh retains identity 1',async()=>{
 api.generate.mockRejectedValueOnce(new Error('ack lost'));
 saved.set('opaque-0',{selectionRef:'opaque-0',token:{instanceId:'t2v',revision:1},inputs:[],assets});
 await start();fireEvent.click(screen.getByRole('button',{name:'生成'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成'}).hasAttribute('disabled')).toBe(false));
 saved.set('opaque-0',{selectionRef:'opaque-0',token:{instanceId:'t2v',revision:1},inputs:[],assets});
 fireEvent.click(screen.getByText('高级设置'));
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'刷新已保存输入'}));});
 act(()=>{const store=useStudioStore.getState();store.loadCreationDraft(Object.fromEntries(Object.entries(store.values).reverse()),store.draftDirty,store.creationPromptProvenance);});
 await readyToGenerate();fireEvent.click(screen.getByRole('button',{name:'生成'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
 expect(api.generate.mock.calls[1][0].values).toEqual(api.generate.mock.calls[0][0].values);
 expect(api.generate.mock.calls[1][0].submissionIdempotencyKey).toBe(api.generate.mock.calls[0][0].submissionIdempotencyKey);
});

it('uncertain retry after a semantically identical input refresh renews identity 2',async()=>{
 api.generate.mockRejectedValueOnce(new Error('ack lost'));
 saved.set('opaque-0',{selectionRef:'opaque-0',token:{instanceId:'t2v',revision:1},inputs:[],assets});
 await start();fireEvent.click(screen.getByRole('button',{name:'生成'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成'}).hasAttribute('disabled')).toBe(false));
 saved.set('opaque-0',{selectionRef:'opaque-0',token:{instanceId:'t2v',revision:2},inputs:[],assets});
 fireEvent.click(screen.getByText('高级设置'));
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'刷新已保存输入'}));});
 act(()=>{const store=useStudioStore.getState();store.loadCreationDraft(Object.fromEntries(Object.entries(store.values).reverse()),store.draftDirty,store.creationPromptProvenance);});
 await readyToGenerate();fireEvent.click(screen.getByRole('button',{name:'生成'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
 expect(api.generate.mock.calls[1][0].values).toEqual(api.generate.mock.calls[0][0].values);
 expect(api.generate.mock.calls[1][0].submissionIdempotencyKey).not.toBe(api.generate.mock.calls[0][0].submissionIdempotencyKey);
});

it('failed Library append must preserve existing plural draft references',async()=>{
 saved.set('opaque-3',{selectionRef:'opaque-3',token:{instanceId:'refs',revision:1},inputs:[{inputKey:'reference_images',ordinal:0,assetId:'first'},{inputKey:'reference_images',ordinal:1,assetId:'last'}],assets:[...assets,{id:'new-image',name:'New image',mediaKind:'image'}]});
 api.get.mockImplementation((p,s)=>{const c=context(p,s);return Promise.resolve({...c,selectedShot:c.selectedShot?{...c.selectedShot,selectionRef:'opaque-3'}:null});});
 await start();
 api.libraryGet.mockResolvedValue({kind:'asset',asset:{id:'new-image',assetType:'image',category:'source_image',sourceTaskId:null,name:'New image',mimeType:'image/png'}});
 act(()=>useStudioStore.getState().setPendingLibraryIntent({kind:'asset',projectId:'owned',assetId:'new-image',mediaKind:'image'}));
 const apply=await screen.findByRole('button',{name:'应用到参考图片'});
 await waitFor(()=>expect(apply.hasAttribute('disabled')).toBe(false));
 api.inputsSave.mockRejectedValueOnce({code:'SHOT_VIDEO_INPUT_CONFLICT',message:'conflict'});
 fireEvent.click(apply);
 await waitFor(()=>expect(api.inputsSave).toHaveBeenCalledTimes(1));
 await screen.findByText(/镜头输入已被更新/);
 const inputIds=useStudioStore.getState().values.reference_images;
 expect(inputIds,'failed append must not silently delete the two prior draft references').toEqual(expect.objectContaining({assetIds:expect.arrayContaining(['first','last'])}));
 expect(api.generate).not.toHaveBeenCalled();
});
it('successful Library append preserves existing plural formal references',async()=>{
 saved.set('opaque-3',{selectionRef:'opaque-3',token:{instanceId:'refs',revision:1},inputs:[{inputKey:'reference_images',ordinal:0,assetId:'first'},{inputKey:'reference_images',ordinal:1,assetId:'last'}],assets:[...assets,{id:'new-image',name:'New image',mediaKind:'image'}]});
 api.get.mockImplementation((p,s)=>{const c=context(p,s);return Promise.resolve({...c,selectedShot:c.selectedShot?{...c.selectedShot,selectionRef:'opaque-3'}:null});});
 await start();api.libraryGet.mockResolvedValue({kind:'asset',asset:{id:'new-image',assetType:'image',category:'source_image',sourceTaskId:null,name:'New image',mimeType:'image/png'}});
 act(()=>useStudioStore.getState().setPendingLibraryIntent({kind:'asset',projectId:'owned',assetId:'new-image',mediaKind:'image'}));
 const apply=await screen.findByRole('button',{name:'应用到参考图片'});await waitFor(()=>expect(apply.hasAttribute('disabled')).toBe(false));fireEvent.click(apply);
 await waitFor(()=>expect(useStudioStore.getState().pendingLibraryIntent).toBeUndefined());
 expect(useStudioStore.getState().values.reference_images).toEqual({type:'image_assets',assetIds:['first','last','new-image']});
 expect(api.generate).not.toHaveBeenCalled();
});
