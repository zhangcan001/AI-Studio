// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ShotVideoInputsPanel } from "./ShotVideoInputsPanel";
import { invalidateVideoInputs, resetVideoInputInvalidationForTests } from "../../product/videoInputInvalidation";
import type { RecipeViewModel } from "../../types/generation";
import type { ShotVideoInputScope, VideoInputView } from "../../types/shotVideoInput";
const api=vi.hoisted(()=>({inputsGet:vi.fn(),inputsSave:vi.fn(),assetsImport:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{creation:api,library:{list:vi.fn(async()=>({items:[],nextCursor:null,coverage:"complete-category",coverageMessage:""})),tagsList:vi.fn(async()=>[]),imageGet:vi.fn(async()=>[137,80,78,71])}}}));
const scope:ShotVideoInputScope={projectId:"p1",shotId:"s1",workflowVersionId:"v1",recipeId:"fl"};
const recipe:RecipeViewModel={workflowId:"w1",workflowVersionId:"v1",recipeId:"fl",selectionRef:"opaque-fl",name:"FL",category:"video",mode:"FIRST_LAST",persistentInputs:true,fields:[{key:"first_frame",label:"首帧",type:"image",required:true},{key:"last_frame",label:"尾帧",type:"image",required:true}]};
function saved(s=scope):VideoInputView{return {selectionRef:`opaque-${s.recipeId}`,token:{instanceId:`instance-${s.recipeId}`,revision:1},inputs:[{inputKey:"first_frame",ordinal:0,assetId:`${s.recipeId}-first`},{inputKey:"last_frame",ordinal:0,assetId:`${s.recipeId}-last`}],assets:["fl-first","fl-last","other-first","other-last","external-new"].map(id=>({id,name:id,mediaKind:"image"}))};}
beforeEach(()=>{(URL as unknown as {createObjectURL:()=>string}).createObjectURL=()=>"blob:img";vi.clearAllMocks();api.inputsGet.mockImplementation(async s=>saved(s.selectionRef==="opaque-other"?{...scope,recipeId:"other"}:scope));api.inputsSave.mockImplementation(async r=>({...saved(),selectionRef:r.selection.selectionRef,token:{instanceId:"instance-fl",revision:2},inputs:r.inputs}));api.assetsImport.mockResolvedValue({imported:[],failed:[],cancelled:false});});
afterEach(() => { cleanup(); resetVideoInputInvalidationForTests(); });

it("keeps two independent slots, disables readiness for unsaved edits and imports without adopting results",async()=>{
  const ready=vi.fn();render(<ShotVideoInputsPanel scope={scope} recipe={recipe} onReadyChange={ready}/>);
  await waitFor(()=>expect(screen.getByText("fl-last")).toBeTruthy());
  await waitFor(()=>expect(ready).toHaveBeenLastCalledWith(JSON.stringify(scope),true));
  fireEvent.click(screen.getByLabelText("首帧"));fireEvent.click(within(screen.getByRole("dialog")).getByRole("option",{name:/external-new/}));
  expect(screen.getByText("fl-last")).toBeTruthy();
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
  await waitFor(()=>expect(screen.getByText("other-first")).toBeTruthy());
  await act(async()=>finish(saved()));
  expect(screen.getByText("other-last")).toBeTruthy();
  fireEvent.click(screen.getByLabelText("尾帧"));fireEvent.click(within(screen.getByRole("dialog")).getByRole("option",{name:/external-new/}));;
  api.inputsSave.mockRejectedValueOnce({code:"SHOT_VIDEO_INPUT_CONFLICT",message:"刷新后重新保存"});
  fireEvent.click(screen.getByText("保存视频输入"));await screen.findByRole("alert");
  expect(screen.getByText("external-new")).toBeTruthy();
  expect(api.inputsSave.mock.calls[0][0].expected).toEqual({instanceId:"instance-other",revision:1});
});


it("re-fetches inputsGet when formal video inputs are invalidated for this project", async () => {
  render(<ShotVideoInputsPanel scope={scope} recipe={recipe} />);
  await waitFor(() => expect(api.inputsGet).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(screen.getByText("fl-first")).toBeTruthy());
  api.inputsGet.mockResolvedValueOnce({
    ...saved(),
    token: { instanceId: "instance-fl", revision: 9 },
    inputs: [{ inputKey: "first_frame", ordinal: 0, assetId: "external-new" }, { inputKey: "last_frame", ordinal: 0, assetId: "fl-last" }],
  });
  act(() => invalidateVideoInputs("p1"));
  await waitFor(() => expect(api.inputsGet).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(screen.getByText("external-new")).toBeTruthy());
  act(() => invalidateVideoInputs("other"));
  expect(api.inputsGet).toHaveBeenCalledTimes(2);
});
