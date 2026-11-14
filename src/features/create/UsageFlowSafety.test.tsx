import { ResultFileActions } from "../../components/ResultFileActions";
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
beforeEach(()=>{HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};api.results.mockResolvedValue([{assetId:"a",name:"候选",mediaKind:"video",assetExists:true,reviewState:"PENDING",reviewRevision:7,selectedShotIds:[]}]);});
const item={id:"s",name:"镜头",ordinal:0,status:"REVIEW",candidates:[{id:"a",name:"候选",mediaKind:"video",selected:false}],shot:null,run:{ref:{source:"task",id:"t"}}} as StoryboardShot;
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
