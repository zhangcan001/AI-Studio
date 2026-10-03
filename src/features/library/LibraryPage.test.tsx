// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
// @ts-expect-error Node helpers are test-only.
import { readFileSync, readdirSync } from "node:fs";
// @ts-expect-error Node subprocess is test-only.
import { execFileSync } from "node:child_process";
import { LibraryPage } from "./LibraryPage";
import { CreatePage } from "../create/CreatePage";
import { normalLibrary, relationRoute, deletionMessage } from "./libraryModel";
import { validateResumeChildren, parseRoute } from "../../app/routes/resumeAdapter";
import { toLegacyLocation } from "../../app/routes/legacyAdapter";
import { invalidateRuns } from "../../product/runInvalidation";
import { useStudioStore } from "../../stores/studioStore";
import type { LibraryDetail, LibraryList, LibraryCreateIntent } from "../../product/libraryTypes";
import type { CreationContext, GeneratorOption } from "../../product/types";
const api=vi.hoisted(()=>({list:vi.fn(),get:vi.fn(),relationsGet:vi.fn(),versionsGet:vi.fn(),imageGet:vi.fn(),useInCreation:vi.fn(),deletionInspect:vi.fn(),delete:vi.fn(),resourceEdit:vi.fn(),creationGet:vi.fn(),generatorsList:vi.fn(),readinessGet:vi.fn(),generate:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{library:{...api,mediaUrl:()=>"http://fixture.invalid/media"},creation:{get:api.creationGet,generatorsList:api.generatorsList,readinessGet:api.readinessGet,generate:api.generate},run:{}}}));
const resource={kind:"prompt",id:"prompt-a"} as const;
const route={kind:"library",projectId:"project-a",filter:"prompts",resource} as const;
const version={id:"version-a",promptId:"prompt-a",version:1,text:"原始正文",modelVersionId:null,createdAt:"2026-10-02"};
const detail:LibraryDetail={kind:"prompt",prompt:{id:resource.id,projectId:"project-a",kind:"prompt",name:"测试提示词",tags:[],versionCount:1,createdAt:"2026-10-02",updatedAt:"2026-10-02",versions:[version]}};
const page:LibraryList={items:[{resourceRef:resource,title:"测试提示词",subtype:"prompt",createdAt:"2026-10-02",updatedAt:"2026-10-02",thumbnailAvailable:false}],nextCursor:null,coverage:"keyset-page",coverageMessage:"真实分类分页"};
const generator:GeneratorOption={selectionRef:"exact-pair",name:"H3",mode:"FL2V",mediaKind:"video",version:"1",availability:true,availabilityReason:null,recommended:true,fields:[{key:"prompt",type:"textarea",label:"提示词",required:true,default:""},{key:"first_frame",type:"image",label:"首帧",required:true},{key:"last_frame",type:"image",label:"尾帧",required:true},{key:"reference_video",type:"video",label:"参考视频",required:false}]};
const context:CreationContext={projectId:"project-a",projectName:"A",stage:"video",shots:[{id:"shot-a",name:"镜头一",ordinal:0}],selectedShot:{summary:{id:"shot-a",name:"镜头一",ordinal:0},prompt:"默认",selectionRef:"exact-pair",values:{},referenceAssetIds:[],selectedResultId:null,recentRun:null},candidates:[],mediaInputs:[{id:"image-a",name:"图片",mediaKind:"image",selected:false},{id:"video-a",name:"视频",mediaKind:"video",selected:false}],promptChoices:[]};
beforeEach(()=>{Object.values(api).forEach(mock=>mock.mockReset());useStudioStore.getState().resetDraft();api.list.mockResolvedValue(page);api.get.mockResolvedValue(detail);api.relationsGet.mockResolvedValue([]);api.versionsGet.mockResolvedValue({kind:"prompt",versions:[version]});api.imageGet.mockResolvedValue([1]);api.creationGet.mockResolvedValue(context);api.generatorsList.mockResolvedValue([generator]);api.readinessGet.mockResolvedValue({ready:true,issues:[],fieldErrors:[],actions:[]});URL.createObjectURL=vi.fn(()=>"blob:fixture");URL.revokeObjectURL=vi.fn();});
afterEach(()=>{cleanup();vi.useRealTimers();});
it("phase5_target12 canonical details search actual paging missing resource and project isolation",async()=>{
 const navigate=vi.fn();const cursor={projectId:"project-a",category:"prompts",keyword:null,position:{createdAt:"2026-10-02",id:"prompt-a"}};
 api.list.mockImplementation((_project,query)=>Promise.resolve(query.cursor ? {...page,items:[]} : {...page,nextCursor:cursor}));
 const view=render(<LibraryPage route={route} navigate={navigate}/>);await screen.findAllByText("原始正文",{selector:"p.library-prompt-text"});
 fireEvent.click(screen.getByRole("button",{name:"加载更多"}));await waitFor(()=>expect(api.list).toHaveBeenCalledWith("project-a",expect.objectContaining({cursor})));
 fireEvent.change(screen.getByLabelText("搜索资源名称"),{target:{value:"中文"}});await waitFor(()=>expect(api.list).toHaveBeenCalledWith("project-a",expect.objectContaining({keyword:"中文",cursor:null})));
 api.get.mockRejectedValue({code:"LIBRARY_RESOURCE_NOT_FOUND"});view.rerender(<LibraryPage route={{...route,resource:{kind:"profile",id:"missing"}}} navigate={navigate}/>);await waitFor(()=>expect(navigate).toHaveBeenCalledWith(expect.objectContaining({resource:undefined})));
 api.list.mockResolvedValue({...page,items:[]});view.rerender(<LibraryPage route={{kind:"library",projectId:"project-b"}} navigate={navigate}/>);expect(screen.queryByText("原始正文")).toBeNull();await screen.findByText("当前分类 / 搜索没有资源。");
 expect(parseRoute({...route,filter:"reference-sets"})).toEqual({...route,filter:"reference-sets"});
 expect(await validateResumeChildren(route,{shotIds:async()=>[],runExists:async()=>{},assetExists:async()=>{},resourceExists:async()=>{throw{code:"LIBRARY_RESOURCE_NOT_FOUND"};}})).toEqual({...route,resource:undefined});
});
it("phase5_target13 Library Create typed intents require explicit compatible slots and create no tasks",async()=>{
 const navigate=vi.fn();api.useInCreation.mockResolvedValue({kind:"prompt",projectId:"project-a",promptVersionId:"version-a",text:"复用正文",modelVersionId:null});
 render(<LibraryPage route={route} navigate={navigate}/>);fireEvent.click(await screen.findByRole("button",{name:"用于创作"}));await waitFor(()=>expect(navigate).toHaveBeenCalledWith({kind:"create",projectId:"project-a",stage:"image"}));cleanup();
 const create={kind:"create",projectId:"project-a",shotId:"shot-a",stage:"video"} as const;
 render(<CreatePage route={create} navigate={navigate}/>);await waitFor(()=>expect(useStudioStore.getState().values.prompt).toEqual({type:"string",value:"复用正文"}));expect(useStudioStore.getState().pendingLibraryIntent).toBeUndefined();cleanup();
 for(const [mediaKind,assetId,button] of [["image","image-a","应用到首帧"],["video","video-a","应用到参考视频"]] as const){useStudioStore.getState().setPendingLibraryIntent({kind:"asset",projectId:"project-a",assetId,mediaKind});render(<CreatePage route={create} navigate={navigate}/>);fireEvent.click(await screen.findByRole("button",{name:button}));expect(useStudioStore.getState().values[mediaKind==="image"?"first_frame":"reference_video"]).toMatchObject({assetId});expect(useStudioStore.getState().values.last_frame).not.toMatchObject({assetId});cleanup();}
 const pending:LibraryCreateIntent={kind:"context",projectId:"project-a",resource:{kind:"profile",id:"profile-a"},message:"需要选择应用位置"};useStudioStore.getState().setPendingLibraryIntent(pending);render(<CreatePage route={create} navigate={navigate}/>);await screen.findByText(/尚未自动应用/);expect(useStudioStore.getState().pendingLibraryIntent).toEqual(pending);expect(api.generate).not.toHaveBeenCalled();
 expect(relationRoute("project-a",{kind:"run",runRef:{source:"task",id:"task-a"}})).toEqual({kind:"runs",projectId:"project-a",run:{source:"task",id:"task-a"}});
 expect(relationRoute("project-a",{kind:"shot",id:"shot-a",stage:"video"})).toEqual(create);
});
it("phase5_target14 event refresh typed edits blocked delete warnings and action feedback persist",async()=>{
 expect(deletionMessage("该素材仍被镜头‘sht_secret’选为图片关键帧，请先更换 selected image asset。")).toBe("该素材当前已被镜头选用。请先更换镜头结果后再删除。");
 render(<LibraryPage route={route} navigate={vi.fn()}/>);fireEvent.click(await screen.findByRole("button",{name:"编辑"}));fireEvent.change(screen.getByLabelText("新版本正文"),{target:{value:"未保存草稿"}});
 const before=api.get.mock.calls.length;invalidateRuns("project-a");await waitFor(()=>expect(api.get.mock.calls.length).toBeGreaterThan(before));expect((screen.getByLabelText("新版本正文") as HTMLTextAreaElement).value).toBe("未保存草稿");
 api.resourceEdit.mockRejectedValue({code:"LIBRARY_EDIT_INVALID"});fireEvent.click(screen.getByRole("button",{name:"保存新版本"}));await screen.findByRole("alert");invalidateRuns("project-a");await waitFor(()=>expect(api.get.mock.calls.length).toBeGreaterThan(before+1));expect(screen.getByRole("alert")).toBeTruthy();expect(screen.getByLabelText("新版本正文")).toBeTruthy();
 api.resourceEdit.mockResolvedValue(detail);fireEvent.click(screen.getByRole("button",{name:"保存新版本"}));await waitFor(()=>expect(screen.queryByLabelText("新版本正文")).toBeNull());expect(api.resourceEdit).toHaveBeenCalledWith("project-a",{kind:"prompt",id:"prompt-a",text:"未保存草稿",modelVersionId:null});
 api.deletionInspect.mockResolvedValue({allowed:false,blockers:["历史生成任务固定输入"],warnings:[],relations:[],consequences:[]});fireEvent.click(screen.getByRole("button",{name:"删除"}));await screen.findByText("无法删除");expect(screen.queryByRole("button",{name:"确认删除资源"})).toBeNull();expect(api.delete).not.toHaveBeenCalled();
 api.deletionInspect.mockResolvedValue({allowed:true,blockers:[],warnings:["历史仍保留，媒体无法预览"],relations:[],consequences:[]});fireEvent.click(screen.getByRole("button",{name:"删除"}));fireEvent.click(await screen.findByRole("button",{name:"确认删除资源"}));await waitFor(()=>expect(api.delete).toHaveBeenCalledWith("project-a",resource,true));
});
it("phase5_target15 Product boundary command parity and no competing Library authority",()=>{
 expect(execFileSync("node",["scripts/dev088-architecture-guard.mjs"],{encoding:"utf8"})).toContain("PRODUCT_COMMAND_PARITY=PASS");
 for(const file of readdirSync("src/features/library").filter((f:string)=>!f.includes(".test.")&&/\.tsx?$/.test(f))){expect(readFileSync(`src/features/library/${file}`,"utf8")).not.toMatch(/services\/(tauriClient|ipc)|@tauri-apps\/api|SELECT\s|INSERT\s|create\(.*zustand/);}
 for(const file of readdirSync("src-tauri/src/application/product/library_facade")){expect(readFileSync(`src-tauri/src/application/product/library_facade/${file}`,"utf8")).not.toMatch(/sqlx::query|SqlitePool|LibraryRepository/);}
 expect(readdirSync("src/stores")).not.toContain("libraryStore.ts");expect(readdirSync("src-tauri/migrations").some((f:string)=>f.startsWith("043"))).toBe(false);
// Like the Create boundary target, this invokes the full architecture guard.
// Phase8 adds all backend production files; keep every assertion with a bounded
// static-scan budget rather than applying a UI interaction's default 5 seconds.
},15000);
it("phase5_target16 one normal entry real page legacy rollback and responsive structure",async()=>{
 expect(normalLibrary(route)).toBe(true);expect(normalLibrary({...route,filter:"advanced-assets"})).toBe(false);expect(normalLibrary(route)).toBe(true);
 expect(toLegacyLocation({...route,filter:"advanced-assets"}).workspace).toBe("assets");expect(toLegacyLocation({...route,filter:"advanced-prompts"}).workspace).toBe("prompts");
 const app=readFileSync("src/app/App.tsx","utf8");expect(app).toContain('<LibraryPage key={activeProject.id}');expect(app).toContain('route.filter === "advanced-assets"');expect(app).toContain('route.filter === "advanced-prompts"');
 for(const path of ["src/features/assets/AssetWorkspace.tsx","src/features/prompts/PromptStudio.tsx","src/features/assets/ConsistencyProfileLibrary.tsx","src/features/assets/ReferenceSetEditor.tsx"])expect(readFileSync(path,"utf8").length).toBeGreaterThan(0);
 render(<LibraryPage route={{kind:"library",projectId:"project-a"}} navigate={vi.fn()}/>);await screen.findByRole("button",{name:/测试提示词/});expect(screen.getByRole("navigation",{name:"资源分类"})).toBeTruthy();expect(screen.getByRole("button",{name:"近期资源"})).toBeTruthy();expect(screen.queryByText("全部资源")).toBeNull();
 expect(readFileSync("src/features/library/LibraryPage.css","utf8")).toContain(".v3-shell:has(.library-page)");
});
