// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CreatePage } from "./CreatePage";
import { useStudioStore } from "../../stores/studioStore";
import type { CreationContext, GeneratorOption } from "../../product/types";
import type { VideoInputView } from "../../types/shotVideoInput";
import { promptEditor, promptText, setPrompt } from "./composerTestUtils";
import { aspectOf, composerReferences, serializePrompt } from "./CreateComposer";

const api = vi.hoisted(() => ({ get:vi.fn(),generatorsList:vi.fn(),inputsGet:vi.fn(),inputsSave:vi.fn(),assetsImport:vi.fn(),readinessGet:vi.fn(),generate:vi.fn(),selectResult:vi.fn(),runGet:vi.fn(),retry:vi.fn(),libraryGet:vi.fn(),imageGet:vi.fn(),mediaUrl:vi.fn() }));
vi.mock("../../product/client",()=>({productClient:{creation:api,run:{get:api.runGet,retry:api.retry,list:vi.fn(async()=>({items:[]}))},library:{get:api.libraryGet,imageGet:api.imageGet}}}));
const route = {kind:"create",projectId:"owned",shotId:"one",stage:"video"} as const;
const runtime = {status:"CONNECTED",endpoint:"http://fixture.invalid",devices:[],runtimeGeneration:1} as const;
const prompt = {type:"textarea",key:"prompt",label:"Prompt",required:true,default:""} as const;
const duration = {type:"integer",key:"duration_seconds",label:"Duration",required:true,default:5,min:4,max:15,step:1} as const;
const presets = [{id:"landscape",label:"16:9 横屏",width:1344,height:768},{id:"portrait",label:"9:16 竖屏",width:768,height:1344}];
const options = [
  {mode:"T2V",fields:[prompt,duration,{type:"integer",key:"width",label:"Width",required:true,default:1344},{type:"integer",key:"height",label:"Height",required:true,default:768}],resolutionPresets:presets},
  {mode:"I2V",fields:[prompt,{type:"image",key:"first_frame",label:"First",required:true}]},
  {mode:"REF2VA",fields:[prompt,{type:"images",key:"reference_images",label:"Images",required:false,minItems:0,maxItems:9}]},
].map((o,i)=>({...o,selectionRef:`opaque-${i}`,name:"H3",version:"1",mediaKind:"video",availability:true,availabilityReason:null,recommended:i===0,persistentInputs:true})) as unknown as GeneratorOption[];
const assets = [{id:"first",name:"外部首帧",mediaKind:"image"},{id:"second",name:"第二张",mediaKind:"image"}] satisfies VideoInputView["assets"];
let saved:Map<string,VideoInputView>;
const nav = vi.fn();
function context(projectId=route.projectId):CreationContext { return {projectId,projectName:"Owned",stage:"video",shots:[{id:"one",name:"镜头一",ordinal:0}],selectedShot:{summary:{id:"one",name:"one",ordinal:0},prompt:"",selectionRef:null,values:{},referenceAssetIds:[],selectedResultId:null,recentRun:null},candidates:[],mediaInputs:[],promptChoices:[]} as CreationContext; }
function view(selectionRef:string):VideoInputView { return saved.get(selectionRef)??{selectionRef,token:null,inputs:[],assets}; }
beforeEach(()=>{
  vi.resetAllMocks();useStudioStore.getState().resetDraft();saved=new Map();sessionStorage.clear();
  vi.spyOn(window,"confirm").mockReturnValue(true);
  api.get.mockImplementation(p=>Promise.resolve(context(p)));api.generatorsList.mockResolvedValue(options);
  api.inputsGet.mockImplementation(s=>Promise.resolve(view(s.selectionRef)));
  api.inputsSave.mockImplementation(async r=>{const next={...view(r.selection.selectionRef),token:{instanceId:"receipt",revision:(saved.get(r.selection.selectionRef)?.token?.revision??0)+1},inputs:r.inputs};saved.set(r.selection.selectionRef,next);return next;});
  api.assetsImport.mockResolvedValue({imported:[],failed:[],cancelled:false});api.readinessGet.mockResolvedValue({ready:true,issues:[],fieldErrors:[],actions:[]});
  api.generate.mockResolvedValue({accepted:true,runRef:{source:"queue-batch",id:"r1"},startOutcome:"QUEUED",startIssue:null});
  api.runGet.mockResolvedValue({ref:{source:"queue-batch",id:"r1"},projectId:"owned",title:"run",status:"QUEUED",phase:"QUEUED",createdAt:"",updatedAt:"",progress:{total:1,succeeded:0,failed:0,cancelled:0},recoverability:{retryItemIds:[],reviewRequired:0},resultsSummary:[],errorSummary:null,preferredParent:null,availableActions:[]});
  api.mediaUrl.mockImplementation((p,id)=>`http://fixture.invalid/${p}/${id}`);api.imageGet.mockResolvedValue([137,80,78,71]);
  (URL as unknown as {createObjectURL:()=>string}).createObjectURL=()=>"blob:img";
});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
const composer = () => screen.getByRole("region",{name:"创作输入框"});
async function start(ref?:string) {
  render(<CreatePage route={route} runtime={{...runtime,devices:[]}} navigate={nav}/>);await screen.findByLabelText("选择生成器");
  if (ref) { fireEvent.change(screen.getByLabelText("选择生成器"),{target:{value:ref}});await waitFor(()=>expect(api.inputsGet).toHaveBeenLastCalledWith({projectId:"owned",shotId:"one",selectionRef:ref})); }
  await waitFor(()=>expect(composer().getAttribute("data-inputs")).toBe("saved"));
}
const persisted = () => api.inputsSave.mock.calls[api.inputsSave.mock.calls.length-1]?.[0].inputs;
async function ready() { await screen.findByText("可以入队生成"); await waitFor(()=>expect(screen.getByRole("button",{name:"生成"}).hasAttribute("disabled")).toBe(false)); }

it("is the single input surface: no legacy prompt/media/parameter panels or second generate button",async()=>{
  await start("opaque-1");
  expect(screen.queryByRole("button",{name:"保存视频输入"})).toBeNull();
  expect(screen.queryByText("本次生成素材")).toBeNull();expect(screen.queryByText("镜头视频输入")).toBeNull();
  expect(document.querySelectorAll("textarea#create-field-prompt").length).toBe(0);
  expect(screen.getAllByRole("button",{name:"生成"})).toHaveLength(1);
  expect(within(composer()).getByRole("textbox",{name:"提示词"})).toBeTruthy();
});
it("+ adds a shot image into the generator input, persists it via OCC save and shows the Image N thumbnail; remove persists too",async()=>{
  await start("opaque-1");
  fireEvent.click(screen.getByRole("button",{name:"添加参考素材"}));
  fireEvent.click(within(screen.getByRole("dialog",{name:"添加参考素材"})).getByRole("button",{name:"外部首帧"}));
  await waitFor(()=>expect(persisted()).toEqual([{inputKey:"first_frame",ordinal:0,assetId:"first"}]));
  await waitFor(()=>expect(composer().getAttribute("data-inputs")).toBe("saved"));
  const thumb=screen.getByRole("button",{name:"插入 Image 1"});
  await waitFor(()=>expect(thumb.querySelector("img")?.getAttribute("src")).toBe("blob:img"));
  expect(api.imageGet).toHaveBeenCalledWith("owned",{kind:"asset",id:"first"});
  expect(useStudioStore.getState().values.first_frame).toEqual({type:"image_asset",assetId:"first"});
  fireEvent.click(screen.getByRole("button",{name:"首帧移除第1项"}));
  await waitFor(()=>expect(persisted()).toEqual([]));
  expect(screen.queryByRole("button",{name:"插入 Image 1"})).toBeNull();
});
it("import from computer binds the imported image into the chosen slot; library opens the image filter",async()=>{
  api.assetsImport.mockResolvedValue({imported:[{id:"new-img",name:"新图",mediaKind:"image"}],failed:[],cancelled:false});
  await start("opaque-2");
  fireEvent.click(screen.getByRole("button",{name:"添加参考素材"}));
  fireEvent.click(within(screen.getByRole("dialog",{name:"添加参考素材"})).getByRole("button",{name:"从电脑导入…"}));
  await waitFor(()=>expect(persisted()).toEqual([{inputKey:"reference_images",ordinal:0,assetId:"new-img"}]));
  await waitFor(()=>expect(screen.getByRole("button",{name:"添加参考素材"}).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button",{name:"添加参考素材"}));
  fireEvent.click(within(screen.getByRole("dialog",{name:"添加参考素材"})).getByRole("button",{name:"在资源库查找"}));
  expect(nav).toHaveBeenLastCalledWith({kind:"library",projectId:"owned",filter:"images"});
  expect(api.generate).not.toHaveBeenCalled();
});
it("clicking a thumbnail or typing @ inserts Image N chips that submit as Image N text",async()=>{
  await start("opaque-2");
  fireEvent.change(screen.getByLabelText("参考图片"),{target:{value:"first"}});fireEvent.change(screen.getByLabelText("参考图片"),{target:{value:"second"}});
  await waitFor(()=>expect(persisted()).toEqual([{inputKey:"reference_images",ordinal:0,assetId:"first"},{inputKey:"reference_images",ordinal:1,assetId:"second"}]));
  setPrompt("角色 ");
  fireEvent.click(screen.getByRole("button",{name:"插入 Image 1"}));
  expect(promptEditor().querySelectorAll(".composer-chip")).toHaveLength(1);
  fireEvent.keyDown(promptEditor(),{key:"@"});
  fireEvent.click(within(screen.getByRole("listbox",{name:"插入参考图"})).getByRole("option",{name:/Image 2/}));
  expect(promptEditor().querySelectorAll(".composer-chip")).toHaveLength(2);
  const text=(useStudioStore.getState().values.prompt as {value:string}).value;
  expect(text).toContain("Image 1");expect(text).toContain("Image 2");expect(text).toMatch(/^角色 /);
  await waitFor(()=>expect(composer().getAttribute("data-inputs")).toBe("saved"));
  await ready();fireEvent.click(screen.getByRole("button",{name:"生成"}));
  await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
  const sent=api.generate.mock.calls[0][0];
  expect(sent.values.prompt.value).toBe(text);
  expect(sent.values.reference_images).toEqual({type:"image_assets",assetIds:["first","second"]});
});
it("prompt edits round-trip through the draft and external prompt changes rebuild the editor with chips",async()=>{
  await start("opaque-2");
  fireEvent.change(screen.getByLabelText("参考图片"),{target:{value:"first"}});
  await waitFor(()=>expect(persisted()).toEqual([{inputKey:"reference_images",ordinal:0,assetId:"first"}]));
  setPrompt("hello");expect(useStudioStore.getState().values.prompt).toEqual({type:"string",value:"hello"});
  useStudioStore.getState().setValue("prompt",{type:"string",value:"Image 1 是角色1"});
  await waitFor(()=>expect(promptEditor().querySelectorAll(".composer-chip")).toHaveLength(1));
  expect(promptText()).toBe("Image 1 是角色1");
});
it("mode dropdown switches generator; resolution, aspect and duration reach the generate payload",async()=>{
  await start();
  fireEvent.change(screen.getByLabelText("画面比例"),{target:{value:"9:16"}});
  expect(useStudioStore.getState().values.width).toEqual({type:"integer",value:768});expect(useStudioStore.getState().values.height).toEqual({type:"integer",value:1344});
  fireEvent.change(screen.getByLabelText("时长（秒）"),{target:{value:"10"}});
  expect(screen.getByLabelText("时长（秒）").getAttribute("max")).toBe("15");
  setPrompt("a cat");
  await ready();fireEvent.click(screen.getByRole("button",{name:"生成"}));
  await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
  const sent=api.generate.mock.calls[0][0];
  expect(sent.selectionRef).toBe("opaque-0");
  expect(sent.values).toMatchObject({width:{type:"integer",value:768},height:{type:"integer",value:1344},duration_seconds:{type:"integer",value:10},prompt:{type:"string",value:"a cat"}});
  expect(api.readinessGet).toHaveBeenCalled();
  fireEvent.change(screen.getByRole("combobox",{name:"视频模式"}),{target:{value:"图生视频"}});
  await waitFor(()=>expect(api.inputsGet).toHaveBeenLastCalledWith({projectId:"owned",shotId:"one",selectionRef:"opaque-1"}));
});
it("submit stays disabled until shared readiness is READY",async()=>{
  api.readinessGet.mockResolvedValue({ready:false,issues:[{code:"INPUT_REQUIRED",details:{field:"first_frame",action:"EDIT_INPUT"}}],fieldErrors:[],actions:[]});
  await start("opaque-1");
  await screen.findByText("请检查输入或运行环境");
  expect(screen.getByRole("button",{name:"生成"}).hasAttribute("disabled")).toBe(true);
  expect(api.generate).not.toHaveBeenCalled();
});
it("numbers images in submit order and serializes chips back to text",()=>{
  const fields=[{type:"image",key:"first_frame",label:"F",required:true},{type:"images",key:"reference_images",label:"R",required:false,minItems:0,maxItems:9}] as never;
  const refs=composerReferences(fields,{first_frame:{type:"image_asset",assetId:"a"},reference_images:{type:"image_assets",assetIds:["b","c"]}} as never,[]);
  expect(refs.map(r=>r.label)).toEqual(["Image 1","Image 2","Image 3"]);
  const root=document.createElement("div");root.append("x ");const chip=document.createElement("span");chip.dataset.ref="2";chip.textContent="ignored";root.append(chip," y");
  expect(serializePrompt(root)).toBe("x Image 2 y");
  expect([aspectOf(1344,768),aspectOf(768,1344),aspectOf(1024,768),aspectOf(768,768)]).toEqual(["16:9","9:16","4:3","1:1"]);
});
