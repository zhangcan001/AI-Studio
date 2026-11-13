// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invalidateRuns } from "../../product/runInvalidation";
import { afterEach, expect, it, vi, beforeEach } from "vitest";
import type { CreationContext, ProductRun } from "../../product/types";
import { StoryboardWorkbench } from "./StoryboardWorkbench";
import { deriveStoryboardStatus, runPercent } from "./storyboardModel";

const api = vi.hoisted(() => ({ get: vi.fn(), workbench: vi.fn(), createShot: vi.fn(), runGet: vi.fn(), generate: vi.fn(), resultsGet: vi.fn(), resultReview: vi.fn(), selectResult: vi.fn(), runList: vi.fn(), generatorsList: vi.fn(), readinessGet: vi.fn(), shotsList: vi.fn(), shotsReorder: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { creation: { shotsList:api.shotsList,shotsReorder:api.shotsReorder,mediaUrl: (p: string, id: string) => `media://${p}/${id}`, get: api.get, workbench: api.workbench, createShot: api.createShot, generate: api.generate, selectResult: api.selectResult, generatorsList: api.generatorsList, readinessGet: api.readinessGet }, run: { list: api.runList, get: api.runGet, resultsGet: api.resultsGet, resultReview: api.resultReview }, storyEntities: { promptSections: vi.fn(async () => ({ style: null, summary: null, soundscape: null, music: null })), list: vi.fn(async () => []), listForShot: vi.fn(async () => []), bindShot: vi.fn(async () => []) } } }));
afterEach(() => { cleanup(); vi.useRealTimers(); vi.clearAllMocks(); window.localStorage.clear(); });
// Legacy card/list tests pin the grid view; the default shot-row view has its own test below.
beforeEach(() => {
  sessionStorage.clear();
  window.localStorage.setItem("ai-studio.storyboard.view", "grid");
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  api.runList.mockResolvedValue({ items: [], nextCursor: null, coverage: "" });
  api.workbench.mockImplementation(async () => workbenchFromContexts());
  api.get.mockImplementation(async (_p: string, shotId: string | null) => context(shotId));
});

const shots = [{ id: "s1", name: "镜头01", ordinal: 1 }, { id: "s2", name: "镜头02", ordinal: 2 }, { id: "s3", name: "镜头03", ordinal: 3 }];
const run = (status: ProductRun["status"], succeeded: number, total: number) => ({ ref: { source: "production-run", id: `r-${status}` }, projectId: "p", title: "run", status, phase: "", createdAt: "", updatedAt: "", progress: { total, succeeded, failed: 0, cancelled: 0 }, recoverability: { retryItemIds: [], reviewRequired: 0 }, resultsSummary: [], errorSummary: null, preferredParent: null, availableActions: [] }) as ProductRun;
function context(shotId: string | null): CreationContext {
  const shot = shots.find(s => s.id === shotId);
  return { projectId: "p", projectName: "P", stage: "video", shots, promptChoices: [], mediaInputs: [],
    selectedShot: shot ? { summary: shot, prompt: `${shot.name} prompt`, selectionRef: null, values: { duration_seconds: { type: "integer", value: 5 } }, referenceAssetIds: [], selectedResultId: shot.id === "s1" ? "a1" : null, recentRun: shot.id === "s2" ? { source: "production-run", id: "r" } : null } : null,
    candidates: shot?.id === "s1" ? [{ id: "a1", name: "a1", mediaKind: "video", selected: true }] : [] };
}

function workbenchFrom(shotFn: (id: string | null) => CreationContext = context, projectId = "p") {
  const base = shotFn(null);
  const items = (base.shots.length ? base.shots : shots).map(s => {
    const ctx = shotFn(s.id);
    return { shot: ctx.selectedShot!, candidates: ctx.candidates };
  }).filter(item => item.shot);
  return { projectId, projectName: "P", stage: "video" as const, shots: base.shots, items, truncated: 0 };
}
function workbenchFromContexts() { return workbenchFrom(context); }
const route = { kind: "create" as const, projectId: "p", stage: "video" as const };

it("UI restore: workbench derives real shot status/stats from the product facade and never generates", async () => {
  api.workbench.mockResolvedValue(workbenchFromContexts());
  api.get.mockImplementation(async (_p: string, shotId: string | null) => context(shotId));
  api.runGet.mockResolvedValue(run("RUNNING", 17, 25));
  const navigate = vi.fn();
  render(<StoryboardWorkbench route={route} navigate={navigate} renderBatch={()=> <dialog open aria-label="批量准备视频"/>} />);
  const stats = await screen.findByLabelText("镜头统计");
  expect(within(stats).getByText("镜头总数").nextSibling?.textContent).toBe("3");
  expect(within(stats).getByText("已选用").nextSibling?.textContent).toBe("1");
  expect(within(stats).getByText("生成中").nextSibling?.textContent).toBe("1");
  expect(within(stats).getByText("待制作").nextSibling?.textContent).toBe("1");
  expect(screen.getAllByText("生成中 68%").length).toBeGreaterThan(0);
  fireEvent.click(within(screen.getByRole("article", { name: "镜头03" })).getByRole("button", { name: "生成" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "create", projectId: "p", stage: "video", shotId: "s3" });
  fireEvent.click(screen.getByRole("button", { name: "批量准备视频" }));
  expect(await screen.findByRole("dialog", { name: "批量准备视频" })).toBeTruthy();
  expect(navigate).not.toHaveBeenCalledWith({ kind: "project-settings", projectId: "p", section: "advanced-shots" });
  expect(api.generate).not.toHaveBeenCalled();
});

it("UI restore: empty project shows honest empty state without demo data", async () => {
  api.workbench.mockResolvedValue({ projectId: "p", projectName: "P", stage: "video", shots: [], items: [], truncated: 0 });
  api.get.mockResolvedValue({ ...context(null), shots: [] });
  render(<StoryboardWorkbench route={route} navigate={vi.fn()} />);
  expect(await screen.findByText("开始你的第一个镜头")).toBeTruthy();
  expect(screen.queryByRole("article")).toBeNull();
});

it("UI restore: status precedence keeps active runs ahead of selection", () => {
  expect(deriveStoryboardStatus(null, [], run("QUEUED", 0, 1))).toBe("QUEUED");
  expect(deriveStoryboardStatus(null, [{ id: "a", name: "a", mediaKind: "video", selected: false }], null)).toBe("REVIEW");
  expect(deriveStoryboardStatus(null, [], run("FAILED", 0, 1))).toBe("FAILED");
  expect(deriveStoryboardStatus(null, [], null)).toBe("TODO");
  expect(runPercent(run("RUNNING", 17, 25))).toBe(68);
  expect(runPercent(null)).toBeNull();
});

function reviewContext(shotId: string | null): CreationContext {
  const base = context(shotId);
  if (shotId !== "s2" || !base.selectedShot) return base;
  return { ...base, candidates: [{ id: "v1", name: "v1", mediaKind: "video", selected: false }] };
}

it("UI restore: grid and list modes render distinct layouts and the choice persists", async () => {
  api.get.mockImplementation(async (_p: string, shotId: string | null) => context(shotId));
  api.runGet.mockResolvedValue(run("SUCCEEDED", 1, 1));
  render(<StoryboardWorkbench route={route} navigate={vi.fn()} />);
  await screen.findByRole("article", { name: "镜头01" });
  expect(document.querySelector('[data-view="grid"]')).toBeTruthy();
  expect(screen.queryByRole("table")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "列表模式" }));
  const table = screen.getByRole("table", { name: "镜头列表（列表模式）" });
  expect(within(table).getAllByRole("columnheader").map(h => h.textContent)).toEqual(["缩略图", "镜头", "状态", "时长", "分辨率", "候选", "操作"]);
  expect(within(table).getByRole("row", { name: "镜头01" })).toBeTruthy();
  expect(screen.queryByRole("article")).toBeNull();
  expect(window.localStorage.getItem("ai-studio.storyboard.view")).toBe("list");
  cleanup();
  render(<StoryboardWorkbench route={route} navigate={vi.fn()} />);
  expect(await screen.findByRole("table")).toBeTruthy();
});

it("UI restore: review panel approves, rejects with comment and selects via existing run/creation IPC", async () => {
  api.workbench.mockResolvedValue(workbenchFrom(reviewContext));
  api.get.mockImplementation(async (_p: string, shotId: string | null) => reviewContext(shotId));
  api.runGet.mockResolvedValue(run("SUCCEEDED", 1, 1));
  api.resultsGet.mockResolvedValue([{ assetId: "v1", name: "v1", mediaKind: "video", assetExists: true, availability: "available", reviewState: "PENDING", reviewRevision: 3, selectedShotIds: [], thumbnailBytes: null }]);
  api.resultReview.mockResolvedValue({}); api.selectResult.mockResolvedValue({});
  render(<StoryboardWorkbench route={route} navigate={vi.fn()} />);
  const card = await screen.findByRole("article", { name: "镜头02" });
  expect(within(screen.getByRole("article", { name: "镜头03" })).queryByRole("button", { name: "审核" })).toBeNull();
  fireEvent.click(within(card).getByRole("button", { name: "审核" }));
  const dialog = await screen.findByRole("dialog", { name: "审核 镜头02" });
  const item = await within(dialog).findByRole("listitem", { name: "候选 v1" });
  expect((within(item).getByRole("button", { name: "退回" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(within(item).getByRole("button", { name: "通过" }));
  await vi.waitFor(() => expect(api.resultReview).toHaveBeenCalledWith("p", { runRef: { source: "production-run", id: "r-SUCCEEDED" }, assetId: "v1", decision: "APPROVED", comment: "", expectedRevision: 3 }));
  fireEvent.change(within(dialog).getByLabelText("审核说明"), { target: { value: "构图不对" } });
  await vi.waitFor(() => expect((within(item).getByRole("button", { name: "退回" }) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(within(item).getByRole("button", { name: "退回" }));
  await vi.waitFor(() => expect(api.resultReview).toHaveBeenLastCalledWith("p", expect.objectContaining({ decision: "REJECTED", comment: "构图不对" })));
  await vi.waitFor(() => expect((within(item).getByRole("button", { name: "选为结果" }) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(within(item).getByRole("button", { name: "选为结果" }));
  await vi.waitFor(() => expect(api.selectResult).toHaveBeenCalledWith("p", "s2", "video", "v1"));
  expect(api.generate).not.toHaveBeenCalled();
});

function idleContext(shotId: string | null, withTask: boolean): CreationContext {
  const base = context(shotId);
  if (!base.selectedShot) return base;
  return { ...base, candidates: [], selectedShot: { ...base.selectedShot, selectedResultId: null, recentRun: withTask && shotId === "s3" ? { source: "production-run", id: "t1" } : null } };
}

it("D3: a waiting queue item without a Task keeps a bounded poll until the Task appears", async () => {
  vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
  const interval = vi.spyOn(window, "setInterval");
  try {
    let task = false;
    api.workbench.mockImplementation(async () => workbenchFrom(id => idleContext(id, task)));
    api.runList.mockResolvedValue({ items: [run("QUEUED", 0, 1)], nextCursor: null, coverage: "" });
    api.runGet.mockImplementation(async (_p, ref) => ref.id === "r-QUEUED" ? { ...run("QUEUED",0,1),detail:{sources:[],inputs:task?[]:[{shotId:"s3",taskId:null,itemId:"pending",executionStatus:"PENDING"}]}} : run("RUNNING",0,1));
    await act(async () => { render(<StoryboardWorkbench route={route} navigate={vi.fn()} />); });
    expect(screen.getByRole("article", { name: "镜头03" })).toBeTruthy();
    expect(interval).toHaveBeenCalledWith(expect.any(Function), 8000);
    expect(api.runGet).toHaveBeenCalledWith("p",{source:"production-run",id:"r-QUEUED"});
    expect(within(screen.getByRole("article",{name:"镜头03"})).getByText("排队中")).toBeTruthy();
    task = true;
    const before = api.workbench.mock.calls.length;
    await act(async () => { await vi.advanceTimersByTimeAsync(7999); });
    expect(api.workbench.mock.calls.length).toBe(before);
    await act(async () => { await vi.advanceTimersByTimeAsync(1); });
    expect(api.workbench.mock.calls.length).toBeGreaterThan(before);
    expect(api.runGet).toHaveBeenCalledWith("p", { source: "production-run", id: "t1" });
    expect(api.generate).not.toHaveBeenCalled();
  } finally { interval.mockRestore(); }
});

it("D3: run invalidation for this project refreshes the workbench without any GENERATING shot", async () => {
  let task = false;
  api.workbench.mockImplementation(async () => workbenchFrom(id => idleContext(id, task)));
  api.runList.mockResolvedValue({ items: [], nextCursor: null, coverage: "" });
  api.runGet.mockResolvedValue(run("RUNNING", 0, 1));
  render(<StoryboardWorkbench route={route} navigate={vi.fn()} />);
  await screen.findByRole("article", { name: "镜头03" });
  task = true;
  act(() => { invalidateRuns("other"); });
  expect(api.runGet).not.toHaveBeenCalled();
  act(() => { invalidateRuns("p"); });
  await waitFor(() => expect(api.runGet).toHaveBeenCalled());
});

it("D4: a late createShot reply never navigates after the workbench unmounted", async () => {
  api.workbench.mockResolvedValue(workbenchFrom(id => idleContext(id, false)));
  api.runList.mockResolvedValue({ items: [], nextCursor: null, coverage: "" });
  let resolve!: (value: { id: string }) => void;
  api.createShot.mockReturnValue(new Promise(r => { resolve = r; }));
  const navigate = vi.fn();
  const view = render(<StoryboardWorkbench route={route} navigate={navigate} />);
  fireEvent.click(await screen.findByRole("button", { name: "+ 新建镜头" }));
  view.unmount();
  await act(async () => { resolve({ id: "s-new" }); });
  expect(api.createShot).toHaveBeenCalledTimes(1);
  expect(navigate).not.toHaveBeenCalled();
});

it("defaults to the shot-row editing view with search, status filters and task queue entry", async () => {
  window.localStorage.clear();
  api.workbench.mockResolvedValue(workbenchFromContexts());
  api.runGet.mockResolvedValue(run("SUCCEEDED", 1, 1));
  const navigate = vi.fn();
  render(<StoryboardWorkbench route={route} navigate={navigate} renderRuns={() => <p>runs</p>} />);
  const rows = await screen.findByRole("region", { name: "分镜编辑" });
  expect(within(rows).getByRole("article", { name: /镜头01/ })).toBeTruthy();
  expect(within(rows).getByLabelText("搜索分镜")).toBeTruthy();
  expect(within(rows).getByRole("group", { name: "状态筛选" })).toBeTruthy();
  fireEvent.click(within(rows).getByRole("button", { name: "任务队列" }));
  expect(await screen.findByRole("dialog", { name: "任务队列与运行记录" })).toBeTruthy();
  expect(navigate).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "分镜模式" }).getAttribute("aria-pressed")).toBe("true");
});

it("background polling preserves existing buttons while manual refresh still locks them", async () => {
  window.localStorage.setItem("ai-studio.storyboard.view", "shots");
  vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
  const current = (id: string | null) => { const c = context(id); return c.selectedShot ? { ...c, selectedShot: { ...c.selectedShot, selectionRef: "video-selection" } } : c; };
  const board = () => workbenchFrom(current);
  api.workbench.mockResolvedValue(board());
  api.runList.mockResolvedValue({ items: [run("RUNNING", 0, 1)], nextCursor: null, coverage: "" });
  api.runGet.mockResolvedValue(run("RUNNING", 0, 1));
  await act(async () => { render(<StoryboardWorkbench route={route} navigate={vi.fn()} />); });
  const button = screen.getAllByRole<HTMLButtonElement>("button", { name: "生成视频" })[0];
  const create = screen.getByRole<HTMLButtonElement>("button", { name: "+ 新建镜头" });
  expect(button.disabled).toBe(false);
  let release!: (value: ReturnType<typeof board>) => void;
  const hold = () => { const pending = new Promise<ReturnType<typeof board>>(r => { release = r; }); api.workbench.mockReturnValue(pending); };
  hold();
  await act(async () => { await vi.advanceTimersByTimeAsync(8000); });
  expect(button.disabled).toBe(false); expect(create.disabled).toBe(false);
  expect(button).toBe(screen.getAllByRole("button", { name: "生成视频" })[0]);
  await act(async () => { release(board()); });
  hold(); fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  expect(button.disabled).toBe(true); expect(create.disabled).toBe(true);
  await act(async () => { release(board()); });
  expect(button.disabled).toBe(false);
  expect(api.generate).not.toHaveBeenCalled();
});

it("coalesces progress reads, retains invalidation during slow IPC, and disposes pending reads on unmount", async () => {
  vi.useFakeTimers();
  api.workbench.mockResolvedValue(workbenchFrom(id => idleContext(id, false)));
  await act(async () => { render(<StoryboardWorkbench route={route} navigate={vi.fn()} />); });
  const reads = () => api.workbench.mock.calls.length;
  const create = screen.getByRole<HTMLButtonElement>("button", { name: "+ 新建镜头" });
  let release!: (value: ReturnType<typeof workbenchFrom>) => void;
  api.workbench.mockImplementation(() => new Promise(r => { release = r; }));
  act(() => { invalidateRuns("other"); for (let i = 0; i < 20; i++) invalidateRuns("p"); });
  expect(reads()).toBe(1);
  await act(async () => { await vi.advanceTimersByTimeAsync(500); });
  expect(reads()).toBe(2); expect(create.disabled).toBe(false);
  act(() => { for (let i = 0; i < 20; i++) invalidateRuns("p"); });
  await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
  expect(reads()).toBe(2);
  api.workbench.mockResolvedValue(workbenchFrom(id => idleContext(id, false)));
  await act(async () => { release(workbenchFrom(id => idleContext(id, false))); await vi.advanceTimersByTimeAsync(500); });
  expect(reads()).toBe(3);
  act(() => { invalidateRuns("p"); });
  cleanup();
  await act(async () => { await vi.advanceTimersByTimeAsync(10000); invalidateRuns("p"); });
  expect(reads()).toBe(3); expect(vi.getTimerCount()).toBe(0);
  expect(api.generate).not.toHaveBeenCalled();
});

it("old-project completion cannot release the new project's read lock or overwrite its shots", async () => {
  vi.useFakeTimers();
  let oldRelease!: (value: ReturnType<typeof workbenchFrom>) => void;
  let nextRelease!: (value: ReturnType<typeof workbenchFrom>) => void;
  api.workbench.mockImplementation((projectId: string) => new Promise(r => { if (projectId === "p") oldRelease = r; else nextRelease = r; }));
  // workbench(projectId, stage, limit) — match client signature via mockImplementation args
  api.workbench.mockImplementation((projectId: string) => new Promise(r => { if (projectId === "p") oldRelease = r; else nextRelease = r; }) as any);
  const view = render(<StoryboardWorkbench route={route} navigate={vi.fn()} />);
  view.rerender(<StoryboardWorkbench route={{ ...route, projectId: "next" }} navigate={vi.fn()} />);
  act(() => invalidateRuns("next"));
  await act(async () => { oldRelease(workbenchFrom(context, "p")); await vi.advanceTimersByTimeAsync(1000); });
  expect(api.workbench.mock.calls.filter(([p]) => p === "next")).toHaveLength(1);
  api.workbench.mockImplementation(async (projectId: string) => workbenchFrom(context, projectId));
  await act(async () => { nextRelease(workbenchFrom(context, "next")); await vi.advanceTimersByTimeAsync(500); });
  expect(screen.getByRole("article", { name: "镜头01" })).toBeTruthy();
  expect(api.workbench.mock.calls.filter(([p]) => p === "next")).toHaveLength(2);
  expect(api.generate).not.toHaveBeenCalled();
});

it("folder import opens in the workbench, preserves navigation and reuses the existing queue route", async () => {
  api.get.mockImplementation(async (_p:string,id:string|null)=>context(id));api.runGet.mockResolvedValue(run("SUCCEEDED",1,1));
  const navigate=vi.fn();const renderFolderImport=vi.fn((props:{onCancel:()=>void;onOpenQueue:(id:string)=>void;onImported:()=>unknown})=><section aria-label="folder pane"><button onClick={()=>props.onOpenQueue("prepared")}>查看新批次</button><button onClick={props.onCancel}>返回分镜</button></section>);
  render(<StoryboardWorkbench route={route} navigate={navigate} renderFolderImport={renderFolderImport} renderRuns={r=><p>{r.run?.id}</p>}/>);
  await screen.findByRole("article",{name:"镜头01"});fireEvent.click(screen.getByRole("button",{name:"导入分镜脚本"}));expect(screen.getByRole("region",{name:"folder pane"})).toBeTruthy();expect(navigate).not.toHaveBeenCalled();
  expect(renderFolderImport.mock.calls[0][0]).toMatchObject({projectId:"p"});fireEvent.click(screen.getByRole("button",{name:"查看新批次"}));expect(screen.getByText("prepared")).toBeTruthy();expect(screen.queryByRole("region",{name:"folder pane"})).toBeNull();expect(api.generate).not.toHaveBeenCalled();
});

it.each([true,false])("import receipt opens the exact Shot through guarded navigation (allowed=%s)", async allowed => {
  api.get.mockImplementation(async (_p:string,id:string|null)=>context(id));api.runGet.mockResolvedValue(run("SUCCEEDED",1,1));
  const navigate=vi.fn(()=>allowed);
  const renderFolderImport=(props:{onOpenShot:(id:string)=>unknown})=><section aria-label="per-shot receipt"><button onClick={()=>void props.onOpenShot("s2")}>编辑导入镜头</button></section>;
  render(<StoryboardWorkbench route={route} navigate={navigate} renderFolderImport={renderFolderImport}/>);
  await screen.findByRole("article",{name:"镜头01"});fireEvent.click(screen.getByRole("button",{name:"导入分镜脚本"}));
  fireEvent.click(screen.getByRole("button",{name:"编辑导入镜头"}));
  expect(navigate).toHaveBeenCalledWith({kind:"create",projectId:"p",stage:"video",shotId:"s2"});
  await waitFor(()=>expect(Boolean(screen.queryByRole("region",{name:"per-shot receipt"}))).toBe(!allowed));
  expect(api.generate).not.toHaveBeenCalled();
});

it("a failed import receipt Shot navigation keeps the receipt open and shows a Chinese error", async () => {
  api.get.mockImplementation(async (_p:string,id:string|null)=>context(id));api.runGet.mockResolvedValue(run("SUCCEEDED",1,1));
  const navigate=vi.fn(async()=>{throw {code:"PROJECT_SCOPE_VIOLATION",message:"镜头不属于当前项目"};});
  const renderFolderImport=(props:{onOpenShot:(id:string)=>unknown})=><section aria-label="per-shot receipt"><button onClick={()=>void props.onOpenShot("s2")}>编辑导入镜头</button></section>;
  render(<StoryboardWorkbench route={route} navigate={navigate} renderFolderImport={renderFolderImport}/>);
  await screen.findByRole("article",{name:"镜头01"});fireEvent.click(screen.getByRole("button",{name:"导入分镜脚本"}));
  fireEvent.click(screen.getByRole("button",{name:"编辑导入镜头"}));
  expect((await screen.findByRole("alert")).textContent).toMatch(/^无法打开镜头：/);
  expect(screen.getByRole("region",{name:"per-shot receipt"})).toBeTruthy();expect(api.generate).not.toHaveBeenCalled();
});

it("opens the exact saved failed-start batch from its real storyboard row, without retry or external navigation", async () => {
  window.localStorage.setItem("ai-studio.storyboard.view", "shots");
  const summary = { id: "s1", name: "镜头01", ordinal: 0 };
  const ref = { source: "queue-batch", id: "saved-start-failed" } as const;
  api.workbench.mockResolvedValue({ projectId: "p", projectName: "P", stage: "video", shots: [summary], truncated: 0, items: [{ shot: { summary, prompt: "Walk slowly", selectionRef: "row-gen", values: {}, referenceAssetIds: [], selectedResultId: null, recentRun: null }, candidates: [] }] });
  api.generatorsList.mockResolvedValue([{ selectionRef: "row-gen", name: "T2V", mode: "T2V", mediaKind: "video", availability: true, fields: [] }]);
  api.readinessGet.mockResolvedValue({ ready: true, issues: [], fieldErrors: [], actions: [] });
  api.generate.mockResolvedValueOnce({ accepted: true, runRef: ref, startOutcome: "FAILED_TO_START", startIssue: { code: "RUNTIME_BLOCKED", message: "blocked", details: {} } });
  const navigate = vi.fn(), renderRuns = vi.fn((r: {run?: {id:string}}) => <p>运行 {r.run?.id}</p>);
  render(<StoryboardWorkbench route={route} navigate={navigate} renderRuns={renderRuns} />);
  const generate = await screen.findByRole("button", { name: "生成视频" });
  await waitFor(() => expect(generate.hasAttribute("disabled")).toBe(false));
  fireEvent.click(generate);
  await screen.findByText("运行已保存，启动失败");
  fireEvent.click(screen.getByRole("button", { name: "查看已保存运行" }));
  expect(renderRuns.mock.calls[0][0]).toEqual({ kind: "runs", projectId: "p", run: ref });
  expect(navigate).not.toHaveBeenCalled();
  expect(api.generate).toHaveBeenCalledTimes(1);
});
it("searches the full lightweight Shot index without fetching rich details for every match",async()=>{
 window.localStorage.setItem('ai-studio.storyboard.view','shots');
 const all=Array.from({length:65},(_,i)=>({id:`all-${i}`,name:`分镜${i+1}`,ordinal:i}));
 api.workbench.mockResolvedValue({...workbenchFrom(context,'p'),shots:all,truncated:62});
 const navigate=vi.fn();render(<StoryboardWorkbench route={route} navigate={navigate}/>);
 const search=await screen.findByLabelText('搜索分镜');fireEvent.change(search,{target:{value:'分镜65'}});
 fireEvent.click(screen.getByRole('button',{name:'打开分镜65'}));expect(navigate).toHaveBeenCalledWith({kind:'create',projectId:'p',stage:'video',shotId:'all-64'});
 expect(api.workbench).toHaveBeenCalledWith('p','video',60);expect(api.generate).not.toHaveBeenCalled();
});

it('AUDIT-R03 A successful new-Shot refresh must survive an already in-flight background read',async()=>{
 window.localStorage.setItem('ai-studio.storyboard.view','shots');api.generatorsList.mockResolvedValue([]);api.runGet.mockResolvedValue(run('SUCCEEDED',1,1));
 const initial=workbenchFromContexts(),summary={id:'new-s4',name:'新镜头04',ordinal:4};
 const updated={...initial,shots:[...initial.shots,summary],items:[...initial.items,{shot:{...initial.items[0].shot,summary,prompt:'',recentRun:null,selectedResultId:null},candidates:[]}]};
 let release!:(value:typeof initial)=>void;const old=new Promise<typeof initial>(r=>release=r);
 api.workbench.mockReset().mockResolvedValueOnce(initial).mockReturnValueOnce(old).mockResolvedValue(updated);api.createShot.mockResolvedValue(summary);
 render(<StoryboardWorkbench route={route} navigate={vi.fn()}/>);
 const add=await screen.findAllByRole('button',{name:'末尾新建分镜'});await waitFor(()=>expect(add[0].hasAttribute('disabled')).toBe(false));
 act(()=>invalidateRuns('p'));await waitFor(()=>expect(api.workbench).toHaveBeenCalledTimes(2));
 fireEvent.click(add[0]);await waitFor(()=>expect(api.createShot).toHaveBeenCalledTimes(1));
 await act(async()=>{release(initial);await old;});
 // Allow a legitimate deferred UI refresh (the existing event debounce is 500ms).
 try { await screen.findByLabelText('新镜头04 名称'); } catch { /* record stale view below; final assertions remain RED */ }
 const beforeManual={reads:api.workbench.mock.calls.length,newShotVisible:!!screen.queryByLabelText('新镜头04 名称')};
 fireEvent.click(screen.getByRole('button',{name:'刷新'}));await screen.findByLabelText('新镜头04 名称');
 console.log('R03 '+JSON.stringify({beforeManual,manualRefreshRestored:true,totalReads:api.workbench.mock.calls.length}));expect(api.generate).not.toHaveBeenCalled();
 expect(beforeManual.reads,'mutation refresh must be queued, not silently dropped by the read lock').toBe(3);
 expect(beforeManual.newShotVisible).toBe(true);
});
it('CONTROL explicit manual refresh displays newly created read-model data',async()=>{
 const initial=workbenchFromContexts(),summary={id:'manual-s4',name:'手动刷新镜头',ordinal:4};api.runGet.mockResolvedValue(run('SUCCEEDED',1,1));
 const updated={...initial,shots:[...initial.shots,summary],items:[...initial.items,{shot:{...initial.items[0].shot,summary,prompt:'',recentRun:null,selectedResultId:null},candidates:[]}]};
 api.workbench.mockReset().mockResolvedValueOnce(initial).mockResolvedValue(updated);
 render(<StoryboardWorkbench route={route} navigate={vi.fn()}/>);await screen.findByRole('article',{name:'镜头01'});fireEvent.click(screen.getByRole('button',{name:'刷新'}));await screen.findByRole('article',{name:'手动刷新镜头'});expect(api.workbench).toHaveBeenCalledTimes(2);expect(api.generate).not.toHaveBeenCalled();
});

it("pages bounded Shot details without generating and preserves the full name index", async()=>{
 const all=Array.from({length:61},(_,i)=>({id:`page-${i}`,name:`分页镜头${i}`,ordinal:i}));
 api.workbench.mockImplementation(async(_p,_stage,_limit,offset=0)=>({projectId:"p",projectName:"P",stage:"video",shots:all,items:all.slice(offset,offset+60).map(summary=>({shot:{summary,prompt:"",selectionRef:null,values:{},referenceAssetIds:[],selectedResultId:null,recentRun:null},candidates:[]})),truncated:Math.max(0,all.length-offset-60)}));
 render(<StoryboardWorkbench route={route} navigate={vi.fn()}/>);
 const next=await screen.findByRole("button",{name:"下一页镜头"});await waitFor(()=>expect(next.hasAttribute("disabled")).toBe(false));fireEvent.click(next);
 await waitFor(()=>expect(api.workbench).toHaveBeenCalledWith("p","video",60,60));expect(await screen.findByRole("article",{name:"分页镜头60"})).toBeTruthy();expect(api.generate).not.toHaveBeenCalled();
});

it("moves only through the authoritative complete Shot ordering, not local display order",async()=>{
 localStorage.setItem("ai-studio.storyboard.view","shots");
 api.shotsList.mockResolvedValue(shots.map(s=>({...s,projectId:"p"})));api.shotsReorder.mockResolvedValue([]);
 render(<StoryboardWorkbench route={route} navigate={vi.fn()}/>);
 const row=await screen.findByRole("article",{name:"分镜 3 镜头02"});fireEvent.click(within(row).getByRole("button",{name:"上移"}));
 await waitFor(()=>expect(api.shotsReorder).toHaveBeenCalledWith("p",["s2","s1","s3"]));expect(api.generate).not.toHaveBeenCalled();
});
it("inserts after the requested Shot using a fresh full order and never starts production",async()=>{
 localStorage.setItem("ai-studio.storyboard.view","shots");api.createShot.mockResolvedValue({id:"inserted",name:"新镜头",ordinal:4});
 api.shotsList.mockResolvedValue([...shots.map(s=>({...s,projectId:"p"})),{id:"inserted",projectId:"p",ordinal:4}]);api.shotsReorder.mockResolvedValue([]);
 render(<StoryboardWorkbench route={route} navigate={vi.fn()}/>);
 const row=await screen.findByRole("article",{name:"分镜 3 镜头02"});fireEvent.click(within(row).getByRole("button",{name:"在此后插入"}));
 await waitFor(()=>expect(api.shotsReorder).toHaveBeenCalledWith("p",["s1","s2","inserted","s3"]));expect(api.generate).not.toHaveBeenCalled();
});

it("retains the insert failure receipt even after the automatic successful list refresh",async()=>{
 localStorage.setItem("ai-studio.storyboard.view","shots");api.createShot.mockResolvedValue({id:"retained",name:"已创建",ordinal:4});api.shotsList.mockRejectedValue({code:"PROJECT_READ_FAILED"});
 render(<StoryboardWorkbench route={route} navigate={vi.fn()}/>);const row=await screen.findByRole("article",{name:"分镜 3 镜头02"});
 const reads=api.workbench.mock.calls.length;fireEvent.click(within(row).getByRole("button",{name:"在此后插入"}));
 await waitFor(()=>expect(api.workbench.mock.calls.length).toBeGreaterThan(reads));
 expect((await screen.findByRole("alert")).textContent).toContain("新镜头已创建；排序未完成");expect(api.generate).not.toHaveBeenCalled();
});
