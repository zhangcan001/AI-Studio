import { ResultFileActions } from "../../components/ResultFileActions";
import { useStudioStore } from "../../stores/studioStore";
// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ShotReviewDialog } from "./ShotReviewDialog";
import { deriveStoryboardStatus } from "./storyboardModel";
import type { StoryboardShot } from "./storyboardModel";
const api=vi.hoisted(()=>({results:vi.fn(),review:vi.fn(),select:vi.fn(),reset:vi.fn(),reveal:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{library:{reveal:api.reveal},run:{resultsGet:api.results,resultReview:api.review,resultReviewReset:api.reset},creation:{selectResult:api.select}}}));
vi.mock("../../services/tauriClient",()=>({resetArtifactReview:api.reset}));
vi.mock("./CreateMediaPreview",()=>({CandidateThumbnail:()=>null}));
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.clearAllMocks();});
beforeEach(()=>{useStudioStore.setState({reviewComments:{}});HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};api.results.mockResolvedValue([{assetId:"a",name:"候选",mediaKind:"video",assetExists:true,reviewState:"PENDING",reviewRevision:7,selectedShotIds:[]}]);});
const item={id:"s",name:"镜头",ordinal:0,status:"REVIEW",candidates:[{id:"a",name:"候选",mediaKind:"video",selected:false}],shot:null,run:{ref:{source:"task",id:"t"}}} as StoryboardShot;
it("restores a closed review draft through the shared project/run/asset Store",async()=>{
 useStudioStore.setState({reviewComments:{}});vi.spyOn(window,"confirm").mockReturnValue(true);
 const props={projectId:"p",item,onClose:vi.fn(),onChanged:vi.fn(),onEdit:vi.fn()};
 const view=render(<ShotReviewDialog {...props}/>);await screen.findByRole("button",{name:"通过"});
 fireEvent.change(screen.getByRole("textbox",{name:"审核说明"}),{target:{value:"保留草稿"}});fireEvent.click(screen.getByRole("button",{name:"关闭"}));view.unmount();
 render(<ShotReviewDialog {...props}/>);await screen.findByRole("button",{name:"通过"});
 expect((screen.getByRole("textbox",{name:"审核说明"}) as HTMLInputElement).value).toBe("保留草稿");
 expect(useStudioStore.getState().reviewComments[JSON.stringify(["p","task","t","a"])]).toBe("保留草稿");
});
it("isolates candidate comments and preserves them on failed review, clearing only successful submission",async()=>{
 api.results.mockResolvedValue(["a","b"].map(assetId=>({assetId,name:assetId,mediaKind:"video",assetExists:true,reviewState:"PENDING",reviewRevision:7,selectedShotIds:[]})));
 const multiple={...item,candidates:[...item.candidates,{...item.candidates[0],id:"b"}]};
 render(<ShotReviewDialog projectId="p" item={multiple} onClose={vi.fn()} onChanged={vi.fn()} onEdit={vi.fn()}/>);
 await screen.findAllByRole("button",{name:"通过"});const fields=screen.getAllByRole("textbox",{name:"审核说明"});
 fireEvent.change(fields[0],{target:{value:"A说明"}});fireEvent.change(fields[1],{target:{value:"B说明"}});
 api.review.mockRejectedValueOnce(new Error("review failed"));fireEvent.click(screen.getAllByRole("button",{name:"通过"})[0]);await screen.findByRole("alert");
 expect((fields[0] as HTMLInputElement).value).toBe("A说明");api.review.mockResolvedValue({});
 fireEvent.click(screen.getAllByRole("button",{name:"通过"})[0]);await waitFor(()=>expect((fields[0] as HTMLInputElement).value).toBe(""));
 expect((fields[1] as HTMLInputElement).value).toBe("B说明");expect(api.select).not.toHaveBeenCalled();
});
it.each([["QUEUED","QUEUED"],["PAUSED","PAUSED"],["FAILED","FAILED"],["RUNNING","GENERATING"]] as const)("shows execution %s independently of an old selected candidate",(status,expected)=>{
 expect(deriveStoryboardStatus({...item.shot!,selectedResultId:"old"},item.candidates,{...item.run!,status})).toBe(expected);
});
it("protects an unsubmitted review comment on Escape and never auto-selects",async()=>{
 const close=vi.fn();vi.spyOn(window,"confirm").mockReturnValue(false);
 render(<ShotReviewDialog projectId="p" item={item} onClose={close} onChanged={vi.fn()} onEdit={vi.fn()}/>);
 await screen.findByRole("button",{name:"通过"});fireEvent.change(screen.getByRole("textbox",{name:"审核说明"}),{target:{value:"还没写完"}});
 fireEvent(screen.getByRole("dialog"),new Event("cancel",{bubbles:true,cancelable:true}));expect(close).not.toHaveBeenCalled();expect(api.select).not.toHaveBeenCalled();expect(api.review).not.toHaveBeenCalled();
});
it("reopens review through the existing revision-checked IPC, not a local status write",async()=>{
 api.results.mockResolvedValue([{assetId:"a",name:"候选",mediaKind:"video",assetExists:true,reviewState:"REJECTED",reviewRevision:8,selectedShotIds:[]}]);vi.spyOn(window,"confirm").mockReturnValue(true);
 render(<ShotReviewDialog projectId="p" item={item} onClose={vi.fn()} onChanged={vi.fn()} onEdit={vi.fn()}/>);
 fireEvent.click(await screen.findByRole("button",{name:"重新审核"}));await waitFor(()=>expect(api.reset).toHaveBeenCalledWith("p",item.run!.ref,"a",8));expect(api.review).not.toHaveBeenCalled();expect(api.select).not.toHaveBeenCalled();
});
it("navigates to next review or edit without submitting work",async()=>{
 const next=vi.fn(),edit=vi.fn();render(<ShotReviewDialog projectId="p" item={item} onClose={vi.fn()} onChanged={vi.fn()} onNext={next} onEdit={edit}/>);
 fireEvent.click(await screen.findByRole("button",{name:"下一个待审核镜头"}));await waitFor(()=>expect(screen.getByRole("button",{name:"修改此镜头"}).hasAttribute("disabled")).toBe(false));fireEvent.click(screen.getByRole("button",{name:"修改此镜头"}));expect(next).toHaveBeenCalledOnce();expect(edit).toHaveBeenCalledOnce();expect(api.review).not.toHaveBeenCalled();expect(api.select).not.toHaveBeenCalled();
});

it("file location uses a project-scoped product request and never selects or generates",async()=>{
 render(<ResultFileActions projectId="owned" assetId="video"/>);fireEvent.click(screen.getByRole("button",{name:"在文件夹中显示"}));
 await waitFor(()=>expect(api.reveal).toHaveBeenCalledWith("owned","video"));expect(api.select).not.toHaveBeenCalled();
});

it("resets file action state for B while A's locate request is pending and ignores late A errors",async()=>{
 let rejectA!:(v:unknown)=>void;api.reveal.mockImplementation((_p:string,id:string)=>id==="A"?new Promise((_ok,fail)=>{rejectA=fail;}):Promise.resolve());
 const view=render(<ResultFileActions projectId="p" assetId="A"/>);fireEvent.click(screen.getByRole("button",{name:"在文件夹中显示"}));expect(screen.getByRole("button",{name:"正在定位…"}).hasAttribute("disabled")).toBe(true);
 view.rerender(<ResultFileActions projectId="p" assetId="B"/>);fireEvent.click(screen.getByRole("button",{name:"在文件夹中显示"}));await waitFor(()=>expect(api.reveal).toHaveBeenCalledWith("p","B"));
 rejectA({code:"INVALID_INPUT"});await waitFor(()=>expect(screen.getByRole("button",{name:"在文件夹中显示"}).hasAttribute("disabled")).toBe(false));expect(screen.queryByRole("alert")).toBeNull();
});
