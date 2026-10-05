// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useLibraryMediaInspection, mediaCheckSummary } from "./useLibraryMediaInspection";
import { MediaIntegrityFacts, ProjectMediaInspection, SingleMediaInspection } from "./MediaIntegrityPanel";
import { LibraryPage } from "./LibraryPage";
import type { MediaIntegrityReport, ResourceRef } from "../../product/libraryTypes";
const api = vi.hoisted(()=>({mediaVerify:vi.fn(),list:vi.fn(),tagsList:vi.fn(),get:vi.fn(),relationsGet:vi.fn(),versionsGet:vi.fn(),imageGet:vi.fn(),thumbnailGet:vi.fn(),delete:vi.fn(),resourceEdit:vi.fn(),useInCreation:vi.fn()}));
vi.mock("../../product/client",()=>({productClient:{library:{...api,mediaUrl:()=>"http://fixture.invalid/media"}}}));
const good = (assetId="a"): MediaIntegrityReport => ({assetId,assetType:"image",boundary:"SAFE",existence:"PRESENT",readability:"READABLE",checksum:"MATCH",preview:"PASS",checkedAt:"2026-10-05T00:00:00+00:00"});
const item = (id:string)=>({resourceRef:{kind:"asset",id},title:`素材 ${id}`,subtype:"image",createdAt:"2026-10-05",updatedAt:"2026-10-05",thumbnailAvailable:false});
const page = (ids:string[],nextCursor: unknown=null)=>({items:ids.map(item),nextCursor,coverage:"keyset-page",coverageMessage:"分页"});
function Harness({projectId="p",resource={kind:"asset",id:"a"} as ResourceRef,open=vi.fn()}:{projectId?:string;resource?:ResourceRef;open?:(id:string)=>void}) {
 const inspection=useLibraryMediaInspection(projectId,resource);
 return <><SingleMediaInspection inspection={inspection} title="素材"/><ProjectMediaInspection inspection={inspection} open={open}/></>;
}
const drain = async()=>{await act(async()=>{for(let i=0;i<30;i++)await Promise.resolve();});};
beforeEach(()=>{vi.resetAllMocks();api.list.mockResolvedValue(page(["a","b","c"]));api.mediaVerify.mockImplementation(async(_p:string,r:ResourceRef)=>good(r.id));api.tagsList.mockResolvedValue([]);api.relationsGet.mockResolvedValue([]);api.versionsGet.mockResolvedValue({kind:"unsupported",reason:"none"});api.imageGet.mockResolvedValue([]);});
afterEach(()=>{cleanup();vi.useRealTimers();});
it("mount and time passing never starts checks; explicit single check renders independent typed facts",async()=>{
 vi.useFakeTimers();render(<Harness/>);await act(async()=>vi.advanceTimersByTime(15000));expect(api.mediaVerify).not.toHaveBeenCalled();expect(api.list).not.toHaveBeenCalled();fireEvent.click(screen.getByRole("button",{name:"检查此媒体"}));await drain();expect(api.mediaVerify).toHaveBeenCalledExactlyOnceWith("p",{kind:"asset",id:"a"});expect(screen.getByText("检查通过")).toBeTruthy();expect(api.delete).not.toHaveBeenCalled();expect(api.resourceEdit).not.toHaveBeenCalled();expect(api.useInCreation).not.toHaveBeenCalled();
});
it("project scan ignores UI filters and lists bounded pages serially, one verification at a time",async()=>{
 let active=0,max=0;api.list.mockResolvedValueOnce(page(Array.from({length:20},(_,i)=>String(i)),{position:{id:"19"}})).mockResolvedValueOnce(page(["20","21"]));api.mediaVerify.mockImplementation(async(_p:string,r:ResourceRef)=>{active++;max=Math.max(max,active);await Promise.resolve();active--;return good(r.id);});
 render(<Harness/>);fireEvent.click(screen.getByRole("button",{name:"检查当前项目媒体"}));await waitFor(()=>expect(screen.getByText(/已检查 22 项/)).toBeTruthy());expect(max).toBe(1);expect(api.mediaVerify).toHaveBeenCalledTimes(22);expect(api.list).toHaveBeenCalledTimes(2);for(const [_p,q] of api.list.mock.calls)expect(q).toMatchObject({category:"media",keyword:null,favoriteOnly:false,tagId:null,limit:20});
});
it("stop allows current asset to finish but schedules neither next asset nor page",async()=>{
 let resolve!:(r:MediaIntegrityReport)=>void;api.mediaVerify.mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));render(<Harness/>);fireEvent.click(screen.getByRole("button",{name:"检查当前项目媒体"}));await waitFor(()=>expect(api.mediaVerify).toHaveBeenCalledTimes(1));fireEvent.click(screen.getByRole("button",{name:"停止检查"}));await act(async()=>resolve(good()));expect(screen.getByText(/已检查 1 项.*已停止/)).toBeTruthy();expect(api.mediaVerify).toHaveBeenCalledTimes(1);expect(api.list).toHaveBeenCalledTimes(1);
});
it("project switch rejects stale publication and stops old scheduling without overlapping old IO",async()=>{
 let resolve!:(r:MediaIntegrityReport)=>void;api.mediaVerify.mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));const view=render(<Harness projectId="p"/>);fireEvent.click(screen.getByRole("button",{name:"检查当前项目媒体"}));await waitFor(()=>expect(api.mediaVerify).toHaveBeenCalledTimes(1));view.rerender(<Harness projectId="q"/>);expect(screen.queryByText(/已检查 1 项/)).toBeNull();expect(screen.getByRole("button",{name:"检查当前项目媒体"}).hasAttribute("disabled")).toBe(true);await act(async()=>resolve(good()));expect(api.mediaVerify).toHaveBeenCalledTimes(1);expect(screen.queryByText("检查通过")).toBeNull();fireEvent.click(screen.getByRole("button",{name:"检查当前项目媒体"}));await waitFor(()=>expect(api.mediaVerify).toHaveBeenCalledTimes(4));expect(api.mediaVerify.mock.calls.slice(1).every(c=>c[0]==="q")).toBe(true);
});
it("asset A-B-A switch cannot revive an old single-check result",async()=>{
 let resolve!:(r:MediaIntegrityReport)=>void;api.mediaVerify.mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));const view=render(<Harness resource={{kind:"asset",id:"a"}}/>);fireEvent.click(screen.getByRole("button",{name:"检查此媒体"}));view.rerender(<Harness resource={{kind:"asset",id:"b"}}/>);view.rerender(<Harness resource={{kind:"asset",id:"a"}}/>);await act(async()=>resolve(good()));expect(screen.queryByText("检查通过")).toBeNull();
});
it.each([
 [{existence:"MISSING",readability:"NOT_CHECKED",checksum:"NOT_CHECKED",preview:"NOT_CHECKED"},"文件缺失"],
 [{readability:"UNREADABLE",checksum:"NOT_CHECKED",preview:"NOT_CHECKED"},"不可读取"],
 [{checksum:"MISMATCH",preview:"PASS"},"校验不一致"],
 [{checksum:"MATCH",preview:"FAIL"},"预览失败"],
 [{preview:"CHECK_UNAVAILABLE"},"预览检查不可用"],
 [{checksum:"INVALID_EXPECTED"},"记录校验值无效"],
 [{boundary:"REJECTED",existence:"NOT_CHECKED",readability:"NOT_CHECKED",checksum:"NOT_CHECKED",preview:"NOT_CHECKED"},"安全边界拒绝"],
] as const)("typed status %s is not healthy and preserves separate facts",(changes,label)=>{
 const report={...good(),...changes};render(<MediaIntegrityFacts report={report}/>);expect(screen.getAllByText(new RegExp(label)).length).toBeGreaterThan(0);expect(screen.queryByText("检查通过")).toBeNull();if(report.preview==="CHECK_UNAVAILABLE"){expect(mediaCheckSummary(report).issue).toBe(false);expect(mediaCheckSummary(report).incomplete).toBe(true);expect(screen.getByText(/不代表媒体文件损坏/)).toBeTruthy();}
});
it("unknown is not healthy and reports remain bounded to50 recent entries",async()=>{
 expect(mediaCheckSummary({...good(),boundary:"NOT_CHECKED",existence:"NOT_CHECKED",readability:"NOT_CHECKED",checksum:"NOT_CHECKED",preview:"NOT_CHECKED"}).label).toBe("部分检查未完成");
 api.list.mockResolvedValueOnce(page(Array.from({length:20},(_,i)=>String(i)),{position:{id:"19"}})).mockResolvedValueOnce(page(Array.from({length:20},(_,i)=>String(i+20)),{position:{id:"39"}})).mockResolvedValueOnce(page(Array.from({length:20},(_,i)=>String(i+40))));render(<Harness/>);fireEvent.click(screen.getByRole("button",{name:"检查当前项目媒体"}));await waitFor(()=>expect(screen.getByText(/已检查 60 项/)).toBeTruthy());expect(screen.getAllByRole("button",{name:/查看使用位置/})).toHaveLength(50);
});
it("Library entry stays explicit and scan result opens existing scoped detail preserving relations",async()=>{
 const navigate=vi.fn();api.get.mockResolvedValue({kind:"asset",asset:{id:"a",name:"素材 a",assetType:"image",width:2,height:2,mimeType:"image/png",sourceTaskId:null}});render(<LibraryPage route={{kind:"library",projectId:"p",filter:"images",resource:{kind:"asset",id:"a"}}} navigate={navigate}/>);await waitFor(()=>expect(screen.getByRole("button",{name:"检查此媒体"})).toBeTruthy());expect(screen.getByRole("heading",{name:"使用位置"})).toBeTruthy();expect(api.mediaVerify).not.toHaveBeenCalled();fireEvent.click(screen.getByRole("button",{name:"检查当前项目媒体"}));await waitFor(()=>expect(screen.getByText(/已检查 3 项/)).toBeTruthy());expect(api.list.mock.calls[api.list.mock.calls.length-1]?.[1]).toMatchObject({category:"media",keyword:null,favoriteOnly:false,tagId:null});fireEvent.click(screen.getByRole("button",{name:"素材 b · 查看使用位置"}));expect(navigate).toHaveBeenCalledWith(expect.objectContaining({kind:"library",projectId:"p",resource:{kind:"asset",id:"b"}}));expect(api.delete).not.toHaveBeenCalled();
});
