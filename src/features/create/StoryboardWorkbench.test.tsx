// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invalidateRuns } from "../../product/runInvalidation";
import { afterEach, expect, it, vi, beforeEach } from "vitest";
import type { CreationContext, ProductRun } from "../../product/types";
import { StoryboardWorkbench } from "./StoryboardWorkbench";
import { deriveStoryboardStatus, runPercent } from "./storyboardModel";

const api = vi.hoisted(() => ({ get: vi.fn(), createShot: vi.fn(), runGet: vi.fn(), generate: vi.fn(), resultsGet: vi.fn(), resultReview: vi.fn(), selectResult: vi.fn(), runList: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { creation: { mediaUrl: (p: string, id: string) => `media://${p}/${id}`, get: api.get, createShot: api.createShot, generate: api.generate, selectResult: api.selectResult }, run: { list: api.runList, get: api.runGet, resultsGet: api.resultsGet, resultReview: api.resultReview } } }));
afterEach(() => { cleanup(); vi.useRealTimers(); vi.clearAllMocks(); window.localStorage.clear(); });
// Legacy card/list tests pin the grid view; the default shot-row view has its own test below.
beforeEach(() => { window.localStorage.setItem("ai-studio.storyboard.view", "grid"); });

const shots = [{ id: "s1", name: "镜头01", ordinal: 1 }, { id: "s2", name: "镜头02", ordinal: 2 }, { id: "s3", name: "镜头03", ordinal: 3 }];
const run = (status: ProductRun["status"], succeeded: number, total: number) => ({ ref: { source: "production-run", id: `r-${status}` }, projectId: "p", title: "run", status, phase: "", createdAt: "", updatedAt: "", progress: { total, succeeded, failed: 0, cancelled: 0 }, recoverability: { retryItemIds: [], reviewRequired: 0 }, resultsSummary: [], errorSummary: null, preferredParent: null, availableActions: [] }) as ProductRun;
function context(shotId: string | null): CreationContext {
  const shot = shots.find(s => s.id === shotId);
  return { projectId: "p", projectName: "P", stage: "video", shots, promptChoices: [], mediaInputs: [],
    selectedShot: shot ? { summary: shot, prompt: `${shot.name} prompt`, selectionRef: null, values: { duration_seconds: { type: "integer", value: 5 } }, referenceAssetIds: [], selectedResultId: shot.id === "s1" ? "a1" : null, recentRun: shot.id === "s2" ? { source: "production-run", id: "r" } : null } : null,
    candidates: shot?.id === "s1" ? [{ id: "a1", name: "a1", mediaKind: "video", selected: true }] : [] };
}
const route = { kind: "create" as const, projectId: "p", stage: "video" as const };

it("UI restore: workbench derives real shot status/stats from the product facade and never generates", async () => {
  api.get.mockImplementation(async (_p: string, shotId: string | null) => context(shotId));
  api.runGet.mockResolvedValue(run("RUNNING", 17, 25));
  const navigate = vi.fn();
  render(<StoryboardWorkbench route={route} navigate={navigate} />);
  const stats = await screen.findByLabelText("镜头统计");
  expect(within(stats).getByText("镜头总数").nextSibling?.textContent).toBe("3");
  expect(within(stats).getByText("已完成").nextSibling?.textContent).toBe("1");
  expect(within(stats).getByText("生成中").nextSibling?.textContent).toBe("1");
  expect(within(stats).getByText("待制作").nextSibling?.textContent).toBe("1");
  expect(screen.getAllByText("生成中 68%").length).toBeGreaterThan(0);
  fireEvent.click(within(screen.getByRole("article", { name: "镜头03" })).getByRole("button", { name: "生成" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "create", projectId: "p", stage: "video", shotId: "s3" });
  fireEvent.click(screen.getByRole("button", { name: "批量生成待制作镜头" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "project-settings", projectId: "p", section: "advanced-shots" });
  expect(api.generate).not.toHaveBeenCalled();
});

it("UI restore: empty project shows honest empty state without demo data", async () => {
  api.get.mockResolvedValue({ ...context(null), shots: [] });
  render(<StoryboardWorkbench route={route} navigate={vi.fn()} />);
  expect(await screen.findByText("开始你的第一个镜头")).toBeTruthy();
  expect(screen.queryByRole("article")).toBeNull();
});

it("UI restore: status precedence keeps active runs ahead of selection", () => {
  expect(deriveStoryboardStatus(null, [], run("QUEUED", 0, 1))).toBe("GENERATING");
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
  let task = false;
  api.get.mockImplementation(async (_p: string, shotId: string | null) => idleContext(shotId, task));
  api.runList.mockResolvedValue({ items: [run("QUEUED", 0, 1)], nextCursor: null, coverage: "" });
  api.runGet.mockResolvedValue(run("RUNNING", 0, 1));
  render(<StoryboardWorkbench route={route} navigate={vi.fn()} />);
  await screen.findByRole("article", { name: "镜头03" });
  expect(api.runGet).not.toHaveBeenCalled();
  task = true;
  const before = api.get.mock.calls.length;
  await act(async () => { vi.advanceTimersByTime(8000); });
  for (let i = 0; i < 20 && !api.runGet.mock.calls.length; i++) await act(async () => { await new Promise(r => setTimeout(r, 10)); });
  expect(api.get.mock.calls.length).toBeGreaterThan(before);
  expect(api.runGet).toHaveBeenCalledWith("p", { source: "production-run", id: "t1" });
});

it("D3: run invalidation for this project refreshes the workbench without any GENERATING shot", async () => {
  let task = false;
  api.get.mockImplementation(async (_p: string, shotId: string | null) => idleContext(shotId, task));
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
  api.get.mockImplementation(async (_p: string, shotId: string | null) => idleContext(shotId, false));
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
  api.get.mockImplementation(async (_p: string, shotId: string | null) => context(shotId));
  api.runGet.mockResolvedValue(run("SUCCEEDED", 1, 1));
  const navigate = vi.fn();
  render(<StoryboardWorkbench route={route} navigate={navigate} />);
  const rows = await screen.findByRole("region", { name: "分镜编辑" });
  expect(within(rows).getByRole("article", { name: /镜头01/ })).toBeTruthy();
  expect(within(rows).getByLabelText("搜索分镜")).toBeTruthy();
  expect(within(rows).getByRole("group", { name: "状态筛选" })).toBeTruthy();
  fireEvent.click(within(rows).getByRole("button", { name: "任务队列" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "runs", projectId: route.projectId });
  expect(screen.getByRole("button", { name: "分镜模式" }).getAttribute("aria-pressed")).toBe("true");
});
