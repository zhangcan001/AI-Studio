// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ShotView } from "../../types/shot";
import { StoryboardBatchDialog } from "./StoryboardBatchDialog";
import { ShotBulkConfigPanel } from "./ShotBulkConfigPanel";
const api=vi.hoisted(()=>({list:vi.fn(),assign:vi.fn(),preflight:vi.fn(),admit:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{creation:{shotsList:api.list,shotsPromptAssign:api.assign}}}));
vi.mock("../../services/tauriClient",()=>({getProjectProductionPreflight:api.preflight,admitProjectProduction:api.admit}));
const shot:ShotView={id:"s",projectId:"p",ordinal:0,name:"one",promptText:"old",status:"DRAFT",imageStatus:"DRAFT",videoStatus:"DRAFT",stageConfigs:[],referenceAssets:[],generationLinks:[],createdAt:"",updatedAt:""};
beforeEach(()=>{vi.resetAllMocks();HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};api.list.mockResolvedValue([shot]);api.assign.mockResolvedValue({updatedShotIds:["s"]});api.preflight.mockResolvedValue({projectId:"p",stage:"video",items:[{shotId:"s",status:"READY",contextHash:"h",workflowVersionId:"v",recipeId:"r",blockers:[]} ]});api.admit.mockResolvedValue({projectId:"p",stage:"video",createdCount:1,createdBatchIds:["b"],alreadyPreparedCount:0,skippedIncomplete:0,skippedBlocked:0});});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
it("requires an explicit confirmed prompt save before preparing the persisted video snapshot",async()=>{
 const confirm=vi.spyOn(window,"confirm").mockReturnValue(false);render(<StoryboardBatchDialog projectId="p" onClose={vi.fn()} onQueue={vi.fn()} onShot={vi.fn()}/>);
 await screen.findByText("v / r");fireEvent.click(screen.getByRole("button",{name:"全选"}));const input=screen.getByRole("textbox");fireEvent.change(input,{target:{value:"new prompt"}});
 expect((screen.getByRole("button",{name:"准备视频生产"}) as HTMLButtonElement).disabled).toBe(true);fireEvent.click(screen.getByRole("button",{name:"批量应用提示词"}));expect(api.assign).not.toHaveBeenCalled();expect((input as HTMLTextAreaElement).value).toBe("new prompt");expect(api.admit).not.toHaveBeenCalled();
 confirm.mockReturnValue(true);fireEvent.click(screen.getByRole("button",{name:"批量应用提示词"}));await waitFor(()=>expect(api.assign).toHaveBeenCalledWith("p",["s"],"new prompt"));await waitFor(()=>expect((input as HTMLTextAreaElement).value).toBe(""));await waitFor(()=>expect((screen.getByRole("button",{name:"全选"}) as HTMLButtonElement).disabled).toBe(false));
 fireEvent.click(screen.getByRole("button",{name:"全选"}));await waitFor(()=>expect((screen.getByRole("button",{name:"准备视频生产"}) as HTMLButtonElement).disabled).toBe(false));fireEvent.click(screen.getByRole("button",{name:"准备视频生产"}));await waitFor(()=>expect(api.admit).toHaveBeenCalledWith({projectId:"p",stage:"video",shotIds:["s"],allowPartial:false}));expect(screen.queryByRole("button",{name:"图片阶段"})).toBeNull();
});
it("retains a failed prompt draft and never prepares it",async()=>{
 vi.spyOn(window,"confirm").mockReturnValue(true);api.assign.mockRejectedValue(new Error("save failed"));render(<StoryboardBatchDialog projectId="p" onClose={vi.fn()} onQueue={vi.fn()} onShot={vi.fn()}/>);await screen.findByText("v / r");fireEvent.click(screen.getByRole("button",{name:"全选"}));fireEvent.change(screen.getByRole("textbox"),{target:{value:"keep"}});fireEvent.click(screen.getByRole("button",{name:"批量应用提示词"}));await screen.findByRole("alert");expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("keep");expect(api.admit).not.toHaveBeenCalled();
});
it("does not show a dead prompt editor if the embedding page supplies no save authority",async()=>{render(<ShotBulkConfigPanel projectId="p" shots={[shot]}/>);await screen.findByText("v / r");expect(screen.queryByRole("textbox")).toBeNull();});
