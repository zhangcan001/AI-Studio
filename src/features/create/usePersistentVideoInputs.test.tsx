// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GeneratorOption } from "../../product/types";
import type { VideoInputSelection, VideoInputView } from "../../types/shotVideoInput";
import { useStudioStore } from "../../stores/studioStore";
import { usePersistentVideoInputs } from "./usePersistentVideoInputs";
import type { CreateRoute } from "./createModel";
const api = vi.hoisted(()=>({inputsGet:vi.fn(),inputsSave:vi.fn(),assetsImport:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{creation:api}}));
const route: CreateRoute = { kind:"create",projectId:"p1",shotId:"shot1",stage:"video" };
const generator: GeneratorOption = { selectionRef:"exact-a",name:"FL",version:"2.2",mode:"FIRST_LAST",mediaKind:"video",availability:true,availabilityReason:null,recommended:true,persistentInputs:true,
  fields:[{key:"first_frame",type:"image",label:"首帧",required:true},{key:"last_frame",type:"image",label:"尾帧",required:true}] };
function view(selectionRef="exact-a",revision=1): VideoInputView { return {selectionRef,token:{instanceId:`instance-${selectionRef}`,revision},inputs:[{inputKey:"first_frame",ordinal:0,assetId:`${selectionRef}-first`},{inputKey:"last_frame",ordinal:0,assetId:`${selectionRef}-last`}],assets:[]}; }
beforeEach(()=>{ vi.clearAllMocks(); useStudioStore.getState().loadCreationDraft({prompt:{type:"string",value:"move"}}); api.inputsGet.mockImplementation((s:VideoInputSelection)=>Promise.resolve(view(s.selectionRef))); api.inputsSave.mockImplementation(async r=>({...view(r.selection.selectionRef,2),inputs:r.inputs})); api.assetsImport.mockResolvedValue({imported:[],failed:[],cancelled:false}); });
describe("persistent video input ownership",()=>{
  it("maps independent first/last slots and sends the exact OCC token",async()=>{
    const {result}=renderHook(()=>usePersistentVideoInputs(route,generator,false));
    await waitFor(()=>expect(result.current.ready).toBe(true));
    expect(useStudioStore.getState().values.first_frame).toEqual({type:"image_asset",assetId:"exact-a-first"});
    expect(useStudioStore.getState().values.last_frame).toEqual({type:"image_asset",assetId:"exact-a-last"});
    act(()=>useStudioStore.getState().setValue("first_frame",{type:"image_asset",assetId:"external-new"}));
    expect(result.current.dirty).toBe(true); expect(result.current.ready).toBe(false);
    await act(async()=>{await result.current.save();});
    expect(api.inputsSave).toHaveBeenCalledWith({selection:{projectId:"p1",shotId:"shot1",selectionRef:"exact-a"},expected:{instanceId:"instance-exact-a",revision:1},inputs:[{inputKey:"first_frame",ordinal:0,assetId:"external-new"},{inputKey:"last_frame",ordinal:0,assetId:"exact-a-last"}]});
    expect(result.current.ready).toBe(true);
  });
  it("ignores a late old Recipe load and recovers each Recipe's saved slots",async()=>{
    let finish!: (v:VideoInputView)=>void;
    api.inputsGet.mockImplementationOnce(()=>new Promise<VideoInputView>(resolve=>{finish=resolve;}));
    const {result,rerender}=renderHook(({g})=>usePersistentVideoInputs(route,g,false),{initialProps:{g:generator}});
    rerender({g:{...generator,selectionRef:"exact-b"}});
    await waitFor(()=>expect(result.current.view?.selectionRef).toBe("exact-b"));
    await act(async()=>finish(view("exact-a")));
    expect(useStudioStore.getState().values.last_frame).toEqual({type:"image_asset",assetId:"exact-b-last"});
    rerender({g:generator}); await waitFor(()=>expect(result.current.view?.selectionRef).toBe("exact-a"));
    expect(useStudioStore.getState().values.first_frame).toEqual({type:"image_asset",assetId:"exact-a-first"});
  });
  it("keeps the user's draft on OCC rejection and refreshes only explicitly",async()=>{
    const {result}=renderHook(()=>usePersistentVideoInputs(route,generator,false)); await waitFor(()=>expect(result.current.ready).toBe(true));
    act(()=>useStudioStore.getState().setValue("last_frame",{type:"image_asset",assetId:"my-edit"}));
    api.inputsSave.mockRejectedValueOnce({code:"SHOT_VIDEO_INPUT_CONFLICT",message:"conflict"});
    await act(async()=>{await result.current.save();});
    expect(result.current.error).toContain("镜头输入已被更新"); expect(result.current.view?.token?.revision).toBe(1);
    expect(useStudioStore.getState().values.last_frame).toEqual({type:"image_asset",assetId:"my-edit"});
    api.inputsGet.mockResolvedValueOnce(view("exact-a",3)); await act(async()=>{await result.current.refresh();});
    expect(result.current.view?.token?.revision).toBe(3); expect(result.current.dirty).toBe(false);
  });
  it("imports without filling slots and ignores late import completion after project change",async()=>{
    let finish!: (v:{imported:{id:string;name:string;mediaKind:"image"}[];failed:[];cancelled:false})=>void;
    api.assetsImport.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
    const {result,rerender}=renderHook(({r})=>usePersistentVideoInputs(r,generator,false),{initialProps:{r:route}}); await waitFor(()=>expect(result.current.ready).toBe(true));
    let pending!:Promise<void>; act(()=>{pending=result.current.importAssets(true);});
    rerender({r:{...route,projectId:"p2",shotId:"shot2"}}); await waitFor(()=>expect(result.current.loading).toBe(false));
    await act(async()=>{finish({imported:[{id:"old-import",name:"old",mediaKind:"image"}],failed:[],cancelled:false});await pending;});
    expect(result.current.view?.assets).toEqual([]); expect(api.inputsSave).not.toHaveBeenCalled(); expect(api.assetsImport).toHaveBeenCalledWith("p1",true);
    expect(result.current.busy).toBe(false);
  });
});
