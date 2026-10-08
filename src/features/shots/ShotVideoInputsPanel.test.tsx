// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ShotVideoInputsPanel } from "./ShotVideoInputsPanel";
import type { RecipeViewModel } from "../../types/generation";
import type { ShotVideoInputScope, VideoInputView } from "../../types/shotVideoInput";
const api=vi.hoisted(()=>({inputsGet:vi.fn(),inputsSave:vi.fn(),assetsImport:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{creation:api}}));
const scope:ShotVideoInputScope={projectId:"p1",shotId:"s1",workflowVersionId:"v1",recipeId:"fl"};
const recipe:RecipeViewModel={workflowId:"w1",workflowVersionId:"v1",recipeId:"fl",selectionRef:"opaque-fl",name:"FL",category:"video",mode:"FIRST_LAST",persistentInputs:true,fields:[{key:"first_frame",label:"首帧",type:"image",required:true},{key:"last_frame",label:"尾帧",type:"image",required:true}]};
function saved(s=scope):VideoInputView{return {selectionRef:`opaque-${s.recipeId}`,token:{instanceId:`instance-${s.recipeId}`,revision:1},inputs:[{inputKey:"first_frame",ordinal:0,assetId:`${s.recipeId}-first`},{inputKey:"last_frame",ordinal:0,assetId:`${s.recipeId}-last`}],assets:["fl-first","fl-last","other-first","other-last","external-new"].map(id=>({id,name:id,mediaKind:"image"}))};}
beforeEach(()=>{vi.clearAllMocks();api.inputsGet.mockImplementation(async s=>saved(s.selectionRef==="opaque-other"?{...scope,recipeId:"other"}:scope));api.inputsSave.mockImplementation(async r=>({...saved(),selectionRef:r.selection.selectionRef,token:{instanceId:"instance-fl",revision:2},inputs:r.inputs}));api.assetsImport.mockResolvedValue({imported:[],failed:[],cancelled:false});});
afterEach(cleanup);

it("keeps two independent slots, disables readiness for unsaved edits and imports without adopting results",async()=>{
  const ready=vi.fn();render(<ShotVideoInputsPanel scope={scope} recipe={recipe} onReadyChange={ready}/>);
  await waitFor(()=>expect((screen.getByLabelText("尾帧") as HTMLSelectElement).value).toBe("fl-last"));
  await waitFor(()=>expect(ready).toHaveBeenLastCalledWith(JSON.stringify(scope),true));
  fireEvent.change(screen.getByLabelText("首帧"),{target:{value:"external-new"}});
  expect((screen.getByLabelText("尾帧") as HTMLSelectElement).value).toBe("fl-last");
  await waitFor(()=>expect(ready).toHaveBeenLastCalledWith(JSON.stringify(scope),false));
  fireEvent.click(screen.getByText("导入图片文件夹"));await waitFor(()=>expect(api.assetsImport).toHaveBeenCalledWith("p1",true));
  expect(api.inputsSave).not.toHaveBeenCalled();
  await waitFor(()=>expect((screen.getByText("保存视频输入") as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(screen.getByText("保存视频输入"));
  await waitFor(()=>expect(ready).toHaveBeenLastCalledWith(JSON.stringify(scope),true));
  expect(api.inputsSave).toHaveBeenCalledWith({selection:{projectId:"p1",shotId:"s1",selectionRef:"opaque-fl"},expected:{instanceId:"instance-fl",revision:1},inputs:[{inputKey:"last_frame",ordinal:0,assetId:"fl-last"},{inputKey:"first_frame",ordinal:0,assetId:"external-new"}]});
});

it("ignores a late old Recipe response and leaves OCC failures as explicit unsaved input",async()=>{
  let finish!:(v:VideoInputView)=>void;
  api.inputsGet.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
  const next={...scope,recipeId:"other"};const {rerender}=render(<ShotVideoInputsPanel scope={scope} recipe={recipe}/>);
  rerender(<ShotVideoInputsPanel scope={next} recipe={{...recipe,recipeId:"other",selectionRef:"opaque-other"}}/>);
  await waitFor(()=>expect((screen.getByLabelText("首帧") as HTMLSelectElement).value).toBe("other-first"));
  await act(async()=>finish(saved()));
  expect((screen.getByLabelText("尾帧") as HTMLSelectElement).value).toBe("other-last");
  fireEvent.change(screen.getByLabelText("尾帧"),{target:{value:"external-new"}});
  api.inputsSave.mockRejectedValueOnce({code:"SHOT_VIDEO_INPUT_CONFLICT",message:"刷新后重新保存"});
  fireEvent.click(screen.getByText("保存视频输入"));await screen.findByRole("alert");
  expect((screen.getByLabelText("尾帧") as HTMLSelectElement).value).toBe("external-new");
  expect(api.inputsSave.mock.calls[0][0].expected).toEqual({instanceId:"instance-other",revision:1});
});
