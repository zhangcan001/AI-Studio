// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CreatePage } from "./CreatePage";
import { useStudioStore } from "../../stores/studioStore";
import type { CreationContext, GeneratorOption } from "../../product/types";
import type { VideoInputView } from "../../types/shotVideoInput";
import { promptEditor, promptText, setPrompt } from "./composerTestUtils";
import { aspectOf, composerReferences, serializePrompt } from "./CreateComposer";
import { productClient } from "../../product/client";
import { STORY_ENTITIES_CHANGED } from "../../types/storyEntity";
import { ShotEntityChips } from "./ShotEntityChips";
import type { StoryEntity } from "../../types/storyEntity";

const api = vi.hoisted(() => ({ get:vi.fn(),generatorsList:vi.fn(),inputsGet:vi.fn(),inputsSave:vi.fn(),assetsImport:vi.fn(),readinessGet:vi.fn(),generate:vi.fn(),selectResult:vi.fn(),runGet:vi.fn(),retry:vi.fn(),libraryGet:vi.fn(),imageGet:vi.fn(),mediaUrl:vi.fn() }));
vi.mock("../../product/client",()=>({productClient:{storyEntities:{list:vi.fn(async()=>[]),listForShot:vi.fn(async()=>[]),bindShot:vi.fn(async()=>[]),create:vi.fn(),update:vi.fn(),delete:vi.fn(),get:vi.fn()},creation:api,run:{get:api.runGet,retry:api.retry,list:vi.fn(async()=>({items:[]}))},library:{get:api.libraryGet,imageGet:api.imageGet,list:vi.fn(async()=>({items:[],nextCursor:null})),tagsList:vi.fn(async()=>[])}}}));
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
function pending<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(yes => { resolve = yes; }); return { promise, resolve }; }
const auditHero: StoryEntity = { id: "hero", projectId: "owned", kind: "character", name: "审计角色", description: "", coverAssetId: null, imageAssetIds: [], createdAt: "", updatedAt: "" };
const auditScene: StoryEntity = { ...auditHero, id: "scene", kind: "scene", name: "审计场景" };

it("audit: Generate never guesses a media slot or injects entity covers into saved inputs", async () => {
  const hero = { ...auditHero, coverAssetId: "entity-cover" };
  vi.mocked(productClient.storyEntities.list).mockResolvedValue([hero]);
  vi.mocked(productClient.storyEntities.listForShot).mockResolvedValue([hero]);
  saved.set("opaque-2", { selectionRef: "opaque-2", token: null, inputs: [], assets: [{ id: "explicit-audit-frame", name: "外部首帧", mediaKind: "image" }] });
  await start("opaque-2");
  fireEvent.click(screen.getByLabelText("参考图片"));
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("option", { name: /外部首帧/ }));
  await waitFor(() => expect(persisted()).toEqual([{ inputKey: "reference_images", ordinal: 0, assetId: "explicit-audit-frame" }]));
  await ready();
  const before = api.inputsSave.mock.calls.length;
  fireEvent.click(screen.getByRole("button", { name: "生成" }));
  expect(useStudioStore.getState().values.reference_images).toEqual({ type: "image_assets", assetIds: ["explicit-audit-frame"] });
  await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(1));
  expect(api.inputsSave).toHaveBeenCalledTimes(before);
  expect(api.generate.mock.calls[0][0].values.reference_images).toEqual({ type: "image_assets", assetIds: ["explicit-audit-frame"] });
});

it("fails closed on a binding read error and permits replacement only after retry", async () => {
  vi.mocked(productClient.storyEntities.list).mockResolvedValue([auditHero, auditScene]);
  vi.mocked(productClient.storyEntities.listForShot).mockRejectedValueOnce(new Error("binding read unavailable")).mockResolvedValue([auditHero]);
  vi.mocked(productClient.storyEntities.bindShot).mockResolvedValue([auditHero, auditScene]);
  render(<ShotEntityChips projectId="owned" shotId="read-error" editable />);
  await screen.findByRole("alert");
  const add = await screen.findByRole("checkbox", { name: "场景 · 审计场景" });
  expect(add.hasAttribute("disabled")).toBe(true);
  fireEvent.click(add);
  expect(productClient.storyEntities.bindShot).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "重试读取绑定" }));
  await waitFor(() => expect(add.hasAttribute("disabled")).toBe(false));
  fireEvent.click(add);
  await waitFor(() => expect(productClient.storyEntities.bindShot).toHaveBeenCalledWith("owned", "read-error", ["hero", "scene"]));
});

it("audit: composer cannot replace unknown initial entity bindings", async () => {
  const initial = pending<StoryEntity[]>();
  vi.mocked(productClient.storyEntities.list).mockResolvedValue([auditHero, auditScene]);
  vi.mocked(productClient.storyEntities.listForShot).mockReturnValue(initial.promise);
  vi.mocked(productClient.storyEntities.bindShot).mockResolvedValue([auditHero, auditScene]);
  await start();
  const add = await screen.findByRole("button", { name: "绑定 审计场景" });
  expect(add.hasAttribute("disabled")).toBe(true);
  fireEvent.click(add); expect(productClient.storyEntities.bindShot).not.toHaveBeenCalled();
  await act(async () => { initial.resolve([auditHero]); });
  await waitFor(() => expect(add.hasAttribute("disabled")).toBe(false));
  fireEvent.click(add);
  await waitFor(() => expect(productClient.storyEntities.bindShot).toHaveBeenCalledWith("owned", "one", ["hero", "scene"]));
});

it("audit: a late binding write cannot replace the next Shot's entity state", async () => {
  const write = pending<StoryEntity[]>();
  vi.mocked(productClient.storyEntities.list).mockResolvedValue([auditHero, auditScene]);
  vi.mocked(productClient.storyEntities.listForShot).mockImplementation(async (_p, s) => s === "two" ? [auditScene] : [auditHero]);
  vi.mocked(productClient.storyEntities.bindShot).mockReturnValue(write.promise);
  api.get.mockImplementation((p, s) => { const ctx = context(p); ctx.shots = [{ id: "one", name: "one", ordinal: 0 }, { id: "two", name: "two", ordinal: 1 }]; ctx.selectedShot!.summary = ctx.shots[s === "two" ? 1 : 0]; return Promise.resolve(ctx); });
  const view = render(<CreatePage route={route} runtime={{ ...runtime, devices: [] }} navigate={nav} />);
  const add = await screen.findByRole("button", { name: "绑定 审计场景" });
  await waitFor(() => expect(screen.getByRole("button", { name: "解除绑定 审计角色" })).toBeTruthy());
  fireEvent.click(add);
  await waitFor(() => expect(productClient.storyEntities.bindShot).toHaveBeenCalled());
  view.rerender(<CreatePage route={{ ...route, shotId: "two" }} runtime={{ ...runtime, devices: [] }} navigate={nav} />);
  await waitFor(() => expect(productClient.storyEntities.listForShot).toHaveBeenCalledWith("owned", "two"));
  await screen.findByRole("button", { name: "解除绑定 审计场景" });
  await act(async () => { write.resolve([auditHero, auditScene]); });
  expect(screen.getByRole("button", { name: "绑定 审计角色" }).getAttribute("aria-pressed")).toBe("false");
});

it("audit: editing detail chips never revokes an avatar that is still displayed", async () => {
  const hero = { ...auditHero, coverAssetId: "detail-avatar" };
  vi.mocked(productClient.storyEntities.list).mockResolvedValue([hero, auditScene]);
  vi.mocked(productClient.storyEntities.listForShot).mockResolvedValue([hero]);
  vi.mocked(productClient.storyEntities.bindShot).mockResolvedValue([hero, auditScene]);
  let count = 0; URL.createObjectURL = vi.fn(() => `blob:detail-${++count}`); URL.revokeObjectURL = vi.fn();
  render(<ShotEntityChips projectId="owned" shotId="one" editable />);
  const button = await screen.findByRole("button", { name: "审计角色" });
  await waitFor(() => expect(button.querySelector("img")).toBeTruthy());
  const displayed = button.querySelector("img")!.src;
  fireEvent.click(screen.getByRole("checkbox", { name: "场景 · 审计场景" }));
  await waitFor(() => expect(productClient.storyEntities.bindShot).toHaveBeenCalled());
  await screen.findByTitle("场景 · 审计场景");
  expect(URL.revokeObjectURL).not.toHaveBeenCalledWith(displayed);
});

it("audit: detail chips ignore a late write after switching Shot props", async () => {
  const write = pending<StoryEntity[]>();
  vi.mocked(productClient.storyEntities.list).mockResolvedValue([auditHero, auditScene]);
  vi.mocked(productClient.storyEntities.listForShot).mockImplementation(async (_p, id) => id === "two" ? [auditScene] : [auditHero]);
  vi.mocked(productClient.storyEntities.bindShot).mockReturnValue(write.promise);
  const view = render(<ShotEntityChips projectId="owned" shotId="one" editable />);
  await screen.findByTitle("角色 · 审计角色");
  fireEvent.click(screen.getByRole("checkbox", { name: "场景 · 审计场景" }));
  await waitFor(() => expect(productClient.storyEntities.bindShot).toHaveBeenCalled());
  view.rerender(<ShotEntityChips projectId="owned" shotId="two" editable />);
  await screen.findByTitle("场景 · 审计场景");
  await act(async () => { write.resolve([auditHero, auditScene]); });
  expect(screen.queryByTitle("角色 · 审计角色")).toBeNull();
});
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
  fireEvent.click(within(screen.getByRole("dialog",{name:"添加参考素材"})).getByRole("option",{name:/外部首帧/}));
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
  fireEvent.click(screen.getByLabelText("参考图片"));fireEvent.click(within(screen.getByRole("dialog")).getByRole("option",{name:/外部首帧/}));
  fireEvent.click(screen.getByLabelText("参考图片"));fireEvent.click(within(screen.getByRole("dialog")).getByRole("option",{name:/第二张/}));
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
  fireEvent.click(screen.getByLabelText("参考图片"));fireEvent.click(within(screen.getByRole("dialog")).getByRole("option",{name:/外部首帧/}));
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
it("renders additional required textarea fields from USER recipes", async () => {
  const custom = options.map((o, i) => i === 0 ? { ...o, fields: [...o.fields, { type: "textarea", key: "scene_description", label: "Scene", required: true, default: "" }] } : o);
  api.generatorsList.mockResolvedValue(custom);
  await start();
  expect(screen.getByLabelText("Scene")).toBeTruthy();
});

it("D5: a late reference thumbnail patches the chip image in place and keeps the prompt caret",async()=>{
 let finish:any;
 assets.push({id:'caret-fresh-source',name:'Caret source',mediaKind:'image'});
 saved.set('opaque-2',{selectionRef:'opaque-2',token:{instanceId:'receipt',revision:1},inputs:[{inputKey:'reference_images',ordinal:0,assetId:'caret-fresh-source'}],assets});
 api.imageGet.mockImplementation((_p:string,ref:{id:string})=>ref.id==='caret-fresh-source'?new Promise(resolve=>{finish=resolve;}):Promise.resolve([137,80,78,71]));
 await start('opaque-2');
 await waitFor(()=>expect(finish).toBeTypeOf('function'));
 setPrompt('hello world');
 const editor=promptEditor();editor.focus();
 const text=editor.firstChild!;
 const range=document.createRange();range.setStart(text,5);range.collapse(true);
 const selection=window.getSelection()!;selection.removeAllRanges();selection.addRange(range);
 fireEvent.mouseUp(editor);
 finish([137,80,78,71]);
 await waitFor(()=>expect(screen.getByRole('button',{name:'插入 Image 1'}).querySelector('img')).not.toBeNull());
 expect(editor.firstChild).toBe(text);
 expect(selection.anchorNode).toBe(text);
 expect(selection.anchorOffset).toBe(5);
});

it("lists all project story entities (not only bound), refetches after a Library save and binds one-click to the shot",async()=>{
  const se=productClient.storyEntities as unknown as Record<string,ReturnType<typeof vi.fn>>;
  const hero={id:"e1",projectId:"owned",kind:"character",name:"阿离",description:"",coverAssetId:null,imageAssetIds:[],createdAt:"",updatedAt:""};
  const fresh={...hero,id:"e2",kind:"scene",name:"雨巷"};
  let all=[hero];se.list.mockImplementation(async()=>all);se.listForShot.mockResolvedValue([]);se.bindShot.mockImplementation(async(_p:string,_s:string,ids:string[])=>all.filter(e=>ids.includes(e.id)));
  await start("opaque-1");
  const strip=await screen.findByRole("group",{name:"镜头角色与场景"});
  expect(within(strip).getByRole("button",{name:/阿离/}).getAttribute("aria-pressed")).toBe("false");
  all=[hero,fresh];window.dispatchEvent(new Event(STORY_ENTITIES_CHANGED));
  await within(strip).findByRole("button",{name:/雨巷/});
  fireEvent.click(within(strip).getByRole("button",{name:/雨巷/}));
  await waitFor(()=>expect(se.bindShot).toHaveBeenCalledWith("owned","one",["e2"]));
  await waitFor(()=>expect(within(strip).getByRole("button",{name:/雨巷/}).getAttribute("aria-pressed")).toBe("true"));
  fireEvent.keyDown(promptEditor(),{key:"@"});
  const pick=await screen.findByRole("listbox",{name:"插入角色与场景"});
  expect(within(pick).getAllByRole("option").map(o=>o.textContent?.replace(/^@/,""))).toEqual(["@阿离","@雨巷"]);
});

it("entities without a cover use their first linked image as avatar in the strip and @ picker (live data: cover null, one image)",async()=>{
  const se=productClient.storyEntities as unknown as Record<string,ReturnType<typeof vi.fn>>;
  const e={id:"e1",projectId:"owned",kind:"character",name:"11",description:"",coverAssetId:null,imageAssetIds:["ast_img"],createdAt:"",updatedAt:""};
  se.list.mockResolvedValue([e]);se.listForShot.mockResolvedValue([]);se.bindShot.mockResolvedValue([e]);
  await start("opaque-1");
  const strip=await screen.findByRole("group",{name:"镜头角色与场景"});
  await waitFor(()=>expect(within(strip).getByRole("button",{name:"绑定 11"}).querySelector("img")).toBeTruthy());
  expect(api.imageGet).toHaveBeenCalledWith("owned",expect.objectContaining({id:"ast_img"}));
  fireEvent.keyDown(promptEditor(),{key:"@"});
  const pick=await screen.findByRole("listbox",{name:"插入角色与场景"});
  expect(within(pick).getByRole("option").querySelector("img")).toBeTruthy();
});
