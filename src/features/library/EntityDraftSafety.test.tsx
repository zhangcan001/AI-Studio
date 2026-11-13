// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";


import { LibraryPage } from "./LibraryPage";





import { useStudioStore } from "../../stores/studioStore";
import type { LibraryDetail, LibraryList } from "../../product/libraryTypes";
import type { CreationContext, GeneratorOption } from "../../product/types";
const api=vi.hoisted(()=>({tagsList:vi.fn(),list:vi.fn(),get:vi.fn(),relationsGet:vi.fn(),versionsGet:vi.fn(),imageGet:vi.fn(),useInCreation:vi.fn(),deletionInspect:vi.fn(),delete:vi.fn(),resourceEdit:vi.fn(),creationGet:vi.fn(),generatorsList:vi.fn(),readinessGet:vi.fn(),generate:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{storyEntities:{list:async()=>[]},library:{...api,mediaUrl:()=>"http://fixture.invalid/media"},creation:{get:api.creationGet,generatorsList:api.generatorsList,readinessGet:api.readinessGet,generate:api.generate},run:{}}}));
const resource={kind:"prompt",id:"prompt-a"} as const;
const version={id:"version-a",promptId:"prompt-a",version:1,text:"原始正文",modelVersionId:null,createdAt:"2026-10-02"};
const detail:LibraryDetail={kind:"prompt",prompt:{id:resource.id,projectId:"project-a",kind:"prompt",name:"测试提示词",tags:[],versionCount:1,createdAt:"2026-10-02",updatedAt:"2026-10-02",versions:[version]}};
const page:LibraryList={items:[{resourceRef:resource,title:"测试提示词",subtype:"prompt",createdAt:"2026-10-02",updatedAt:"2026-10-02",thumbnailAvailable:false}],nextCursor:null,coverage:"keyset-page",coverageMessage:"真实分类分页"};
const generator:GeneratorOption={selectionRef:"exact-pair",name:"H3",mode:"FL2V",mediaKind:"video",version:"1",availability:true,availabilityReason:null,recommended:true,fields:[{key:"prompt",type:"textarea",label:"提示词",required:true,default:""},{key:"first_frame",type:"image",label:"首帧",required:true},{key:"last_frame",type:"image",label:"尾帧",required:true},{key:"reference_video",type:"video",label:"参考视频",required:false}]};
const context:CreationContext={projectId:"project-a",projectName:"A",stage:"video",shots:[{id:"shot-a",name:"镜头一",ordinal:0}],selectedShot:{summary:{id:"shot-a",name:"镜头一",ordinal:0},prompt:"默认",selectionRef:"exact-pair",values:{},referenceAssetIds:[],selectedResultId:null,recentRun:null},candidates:[],mediaInputs:[{id:"image-a",name:"图片",mediaKind:"image",selected:false},{id:"video-a",name:"视频",mediaKind:"video",selected:false}],promptChoices:[]};
beforeEach(()=>{Object.values(api).forEach(mock=>mock.mockReset());useStudioStore.getState().resetDraft();api.tagsList.mockResolvedValue([]);api.list.mockResolvedValue(page);api.get.mockResolvedValue(detail);api.relationsGet.mockResolvedValue([]);api.versionsGet.mockResolvedValue({kind:"prompt",versions:[version]});api.imageGet.mockResolvedValue([1]);api.creationGet.mockResolvedValue(context);api.generatorsList.mockResolvedValue([generator]);api.readinessGet.mockResolvedValue({ready:true,issues:[],fieldErrors:[],actions:[]});URL.createObjectURL=vi.fn(()=>"blob:fixture");URL.revokeObjectURL=vi.fn();});
afterEach(()=>{cleanup();vi.useRealTimers();sessionStorage.clear();});

it('AUDIT-F06 switching entity kind must not silently discard a dirty editor',async()=>{
 const confirm=vi.spyOn(window,'confirm').mockReturnValue(false);
 render(<LibraryPage route={{kind:'library',projectId:'project-a',filter:'images'}} navigate={vi.fn()}/>);
 fireEvent.click(screen.getByRole('button',{name:'角色'}));const create=await screen.findByRole('button',{name:'新建角色'});await waitFor(()=>expect(create.hasAttribute('disabled')).toBe(false));fireEvent.click(create);
 fireEvent.change(screen.getByLabelText('角色名称'),{target:{value:'Do not silently discard'}});
 fireEvent.click(screen.getByRole('button',{name:'场景'}));await screen.findByRole('region',{name:'角色管理'});expect(confirm).toHaveBeenCalledTimes(1);expect((screen.getByLabelText('角色名称') as HTMLInputElement).value).toBe('Do not silently discard');
 const afterSwitch={confirmations:confirm.mock.calls.length,originalEditorPresent:!!screen.queryByLabelText('角色名称')};
 fireEvent.click(screen.getByRole('button',{name:'角色'}));await screen.findByRole('region',{name:'角色管理'});
 console.log('F06 '+JSON.stringify({...afterSwitch,restoredEditorPresent:!!screen.queryByLabelText('角色名称')}));
 expect(afterSwitch.confirmations>0||!!screen.queryByLabelText('角色名称'),'dirty entity editor needs a guard or restored draft').toBe(true);
});
it('CONTROL entity kind switch works when no edit is pending',async()=>{
 render(<LibraryPage route={{kind:'library',projectId:'project-a',filter:'images'}} navigate={vi.fn()}/>);fireEvent.click(screen.getByRole('button',{name:'角色'}));await screen.findByRole('region',{name:'角色管理'});fireEvent.click(screen.getByRole('button',{name:'场景'}));await screen.findByRole('region',{name:'场景管理'});
});
