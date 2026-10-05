// @vitest-environment jsdom
import { useState } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
// @ts-expect-error Node helpers are used only in Vitest.
import { readFileSync, readdirSync } from "node:fs";
import { RunsPage } from "./RunsPage";
import { CreatePage } from "../create/CreatePage";
import { normalRuns } from "./runsModel";
import { overviewAction } from "../../app/v3/ProjectOverviewPage";
import { resolveResume } from "../../app/routes/resumeAdapter";
import { invalidateRuns } from "../../product/runInvalidation";
import { useStudioStore } from "../../stores/studioStore";
import type { AppRoute } from "../../app/routes/types";
import type { ProductRun, GeneratorOption, CreationContext, RunResult } from "../../product/types";
const api = vi.hoisted(() => ({ list: vi.fn(), get: vi.fn(), resultsGet: vi.fn(), start: vi.fn(), pause: vi.fn(), cancel: vi.fn(), retry: vi.fn(), resultReview: vi.fn(), creationGet: vi.fn(), generatorsList: vi.fn(), readinessGet: vi.fn(), generate: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { run: api, creation: { get: api.creationGet, generatorsList: api.generatorsList, readinessGet: api.readinessGet, generate: api.generate, mediaUrl: () => "http://fixture.invalid/video" } } }));
const ref = { source: "task", id: "task-secret" } as const;
const run: ProductRun = { ref, projectId: "project-a", title: "生成视频", status: "FAILED", phase: "FAILED", createdAt: "2026-10-02T00:00:00Z", updatedAt: "2026-10-02T00:00:00Z", progress: { total: 1, succeeded: 0, failed: 1, cancelled: 0 }, recoverability: { retryItemIds: [], reviewRequired: 1 }, resultsSummary: [], errorSummary: "请修改输入", preferredParent: null, availableActions: ["EDIT_INPUT"], detail: { sources: [{ id: "shot-a", name: "镜头一", stage: "video" }], inputs: [{ taskId: "task-secret", generatorName: "H3 高质量", selectionRef: "exact-pair", values: { prompt: { type: "string", value: "原始提示词" }, duration_seconds: { type: "integer", value: 5 } }, errorMessage: "时长超出范围", reuseUnavailableReason: null }] } };
const generator: GeneratorOption = { selectionRef: "exact-pair", name: "H3 高质量", mode: "T2V", mediaKind: "video", version: "1", availability: true, availabilityReason: null, recommended: true, fields: [{ key: "prompt", type: "textarea", label: "提示词", default: "", required: true }, { key: "duration_seconds", type: "integer", label: "时长（秒）", default: 1, required: true }] };
const context: CreationContext = { projectId: "project-a", projectName: "Project A", stage: "video", shots: [{ id: "shot-a", name: "镜头一", ordinal: 1 }], selectedShot: { summary: { id: "shot-a", name: "镜头一", ordinal: 1 }, prompt: "默认", selectionRef: "exact-pair", values: {}, referenceAssetIds: [], selectedResultId: null, recentRun: null }, candidates: [], mediaInputs: [], promptChoices: [] };
const result: RunResult = { assetId: "asset-secret", name: "视频结果", mediaKind: "video", assetExists: true, availability: "available", reviewState: "PENDING", reviewRevision: 0, selectedShotIds: [], thumbnailBytes: null };
beforeEach(() => {
  for (const mock of Object.values(api)) mock.mockReset();
  api.list.mockResolvedValue({ items: [run], nextCursor: null, coverage: "FINITE" }); api.get.mockResolvedValue(run); api.resultsGet.mockResolvedValue([result]);
  api.generatorsList.mockImplementation((_p, stage) => Promise.resolve(stage === "video" ? [generator] : [])); api.creationGet.mockResolvedValue(context); api.readinessGet.mockResolvedValue({ ready: true, issues: [], fieldErrors: [], actions: [] });
  useStudioStore.getState().resetDraft();
});
afterEach(() => { cleanup(); vi.useRealTimers(); });
it("phase4_target_9 edit input creates only a scoped Create intent", async () => {
  const navigate = vi.fn(); render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={navigate} />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑这些输入并创建新运行" }));
  await waitFor(() => expect(navigate).toHaveBeenCalledWith({ kind: "create", projectId: "project-a", stage: "video", shotId: "shot-a" }));
  expect(useStudioStore.getState().pendingRunIntent?.selectionRef).toBe("exact-pair"); expect(api.retry).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
  cleanup(); navigate.mockClear();
  api.get.mockResolvedValue({ ...run, detail: { ...run.detail, inputs: [{ ...run.detail!.inputs[0], values: {}, reuseUnavailableReason: "历史输入不完整，将使用默认值重新编辑。" }] } });
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={navigate} />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑这些输入并创建新运行" }));
  await waitFor(() => expect(navigate).toHaveBeenCalled());
  expect(useStudioStore.getState().pendingRunIntent?.values).toEqual({});
  expect(api.generate).not.toHaveBeenCalled(); expect(api.retry).not.toHaveBeenCalled();
});
it("phase4_target_12 invalidates task queue and review projections without changing facts", async () => {
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={vi.fn()} />);
  await screen.findByRole("button", { name: "审核通过" });
  const reads = api.get.mock.calls.length; invalidateRuns("project-b"); expect(api.get.mock.calls.length).toBe(reads);
  api.get.mockResolvedValue({ ...run, status: "PARTIAL" }); invalidateRuns("project-a");
  await screen.findByText("部分完成");
  api.resultReview.mockResolvedValue(undefined); api.resultsGet.mockResolvedValue([{ ...result, reviewState: "APPROVED" }]);
  fireEvent.click(screen.getByRole("button", { name: "审核通过" })); await screen.findByText(/已通过/);
  expect(api.resultReview).toHaveBeenCalledWith("project-a", expect.objectContaining({ runRef: ref, expectedRevision: 0, decision: "APPROVED" }));
  expect(api.cancel).not.toHaveBeenCalled(); expect(api.retry).not.toHaveBeenCalled();
});
it("phase4_target_13 route filters missing locator transport failures and project switch stay separate", async () => {
  for (const filter of ["all", "completed"]) {
    const route = { kind: "runs", projectId: "project-a", run: ref, filter } as const;
    expect(resolveResume({ version: 2, route }, {}, ["project-a"])).toEqual(route);
  }
  const navigate = vi.fn(); const view = render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={navigate} />);
  await screen.findByRole("button", { name: "编辑这些输入并创建新运行" });
  expect(screen.getByText("高级技术详情").parentElement?.hasAttribute("open")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "进行中" })); expect(navigate).toHaveBeenCalledWith(expect.objectContaining({ filter: "active" }));
  api.get.mockResolvedValue({ ...run, status: "QUEUED", availableActions: ["START"] });
  invalidateRuns("project-a");
  api.start.mockRejectedValue({ code: "RUN_START_BLOCKED" });
  // Prove background refresh cannot erase the denied action's reason.
  fireEvent.click(await screen.findByRole("button", { name: "启动运行" }));
  await screen.findByRole("alert");
  const before = api.get.mock.calls.length; invalidateRuns("project-a");
  await waitFor(() => expect(api.get.mock.calls.length).toBeGreaterThan(before));
  expect(screen.getByRole("alert").textContent).toContain("当前无法启动");
  api.get.mockRejectedValue({ code: "RUN_NOT_FOUND" }); view.rerender(<RunsPage route={{ kind: "runs", projectId: "project-a", run: { ...ref, id: "missing" } }} navigate={navigate} />);
  await screen.findByText(/运行已不存在或不可访问/);
  api.list.mockRejectedValue({ code: "INTERNAL_ERROR" }); view.rerender(<RunsPage key="project-b" route={{ kind: "runs", projectId: "project-b" }} navigate={navigate} />);
  await screen.findByRole("alert"); expect(screen.queryByText(/运行已不存在或不可访问/)).toBeNull(); expect(screen.queryByText("H3 高质量 · 输入 1")).toBeNull();
});
it("phase4_target_14 Runs to Create consumes exact input intent without immediate generation", async () => {
  const navigation = vi.fn();
  function Harness() { const [route, setRoute] = useState<AppRoute>({ kind: "runs", projectId: "project-a", run: ref }); const navigate = (next: AppRoute) => { navigation(next); setRoute(next); }; return route.kind === "create" ? <CreatePage route={route} navigate={navigate} /> : route.kind === "runs" ? <RunsPage route={route} navigate={navigate} /> : null; }
  render(<Harness />); fireEvent.click(await screen.findByRole("button", { name: "编辑这些输入并创建新运行" }));
  await screen.findByRole("button", { name: "生成" }); expect(useStudioStore.getState().values.prompt).toEqual({ type: "string", value: "原始提示词" }); expect(useStudioStore.getState().values.duration_seconds).toEqual({ type: "integer", value: 5 }); expect(useStudioStore.getState().pendingRunIntent).toBeUndefined(); expect(api.generate).not.toHaveBeenCalled();
  const acceptedRef = { source: "queue-batch", id: "accepted-owned-run" } as const;
  api.generate.mockResolvedValue({ accepted: true, runRef: acceptedRef, startOutcome: "STARTED", startIssue: null });
  fireEvent.click(screen.getByRole("button", { name: "生成" }));
  fireEvent.click(await screen.findByRole("button", { name: "查看运行详情" }));
  expect(navigation).toHaveBeenLastCalledWith({ kind: "runs", projectId: "project-a", run: acceptedRef });
  expect(api.generate).toHaveBeenCalledTimes(1);
  expect(api.generate).toHaveBeenCalledWith(expect.objectContaining({ projectId: "project-a", selectionRef: "exact-pair", values: expect.objectContaining({ prompt: { type: "string", value: "原始提示词" } }) }));
  // The second scenario still proves that reuse itself never submits.
  api.generate.mockClear();
  cleanup(); useStudioStore.getState().resetDraft();
  api.creationGet.mockResolvedValue({ ...context, selectedShot: null });
  api.get.mockResolvedValue({ ...run, detail: { ...run.detail, sources: [] } });
  render(<Harness />); fireEvent.click(await screen.findByRole("button", { name: "编辑这些输入并创建新运行" }));
  await screen.findByText("选择一个镜头开始创作。");
  expect(useStudioStore.getState().pendingRunIntent?.values.prompt).toEqual({ type: "string", value: "原始提示词" });
  api.creationGet.mockResolvedValue(context);
  fireEvent.change(screen.getByRole("combobox", { name: "镜头" }), { target: { value: "shot-a" } });
  await screen.findByRole("button", { name: "生成" });
  await waitFor(() => expect(useStudioStore.getState().values.prompt).toEqual({ type: "string", value: "原始提示词" }));
  expect(useStudioStore.getState().pendingRunIntent).toBeUndefined();
  expect(api.generate).not.toHaveBeenCalled();
});
it("phase4_target_15 Runs boundary and registered typed commands are guarded", async () => {
  for (const file of readdirSync("src/features/runs").filter((file: string) => !file.includes(".test.") && /\.tsx?$/.test(file))) {
    const code = readFileSync(`src/features/runs/${file}`, "utf8"); expect(code).not.toMatch(/services\/(tauriClient|ipc)|@tauri-apps\/api|create\(.*zustand/);
  }
}, 15_000);
it("phase4_target_16 ordinary Runs entry converges with explicit legacy bridges preserved", () => {
  for (const kind of ["ACTIVE_PRODUCTION", "REVIEW_REQUIRED", "AUTO_RESUMABLE"]) {
    const action = overviewAction("p", { kind, priority: 1, reasonCode: "", reason: "", shotId: null, batchId: "b", taskId: null, assetId: null });
    expect(normalRuns(action.route)).toBe(true);
  }
  expect(normalRuns({ kind: "runs", projectId: "p", run: ref })).toBe(true);
  for (const filter of ["tasks", "production", "review"]) expect(normalRuns({ kind: "runs", projectId: "p", filter })).toBe(true);
  expect(normalRuns({ kind: "runs", projectId: "p" })).toBe(true);
  const app = readFileSync("src/app/App.tsx", "utf8"); // Phase10: retain the normal-route proof at its actual composition owner.
    const pages = readFileSync("src/app/NormalProductPages.tsx", "utf8");
    expect(app).toContain("<NormalProductPages project={activeProject} route={route} navigate={navigate} onDirtyChange={setShotDraftDirty} />");
    expect(pages).toContain("<RunsPage"); expect(app).toContain('route.section === "advanced-tasks"'); expect(app).toContain('["advanced-shots", "advanced-production", "advanced-review"].includes(route.section)'); expect(app).toContain("<TaskHistory"); expect(app).toContain("<ShotWorkspace");
});

it("m2_retry uses only authoritative original snapshot item ids, never the current draft", async () => {
  const ids = ["item-1", "item-2"];
  api.get.mockResolvedValue({ ...run, availableActions: ["RETRY"], recoverability: { retryItemIds: ids, reviewRequired: 0 } });
  api.retry.mockResolvedValue(run);
  useStudioStore.getState().setValue("prompt", { type: "string", value: "unsubmitted different prompt" });
  const before = useStudioStore.getState().values; const navigate = vi.fn();
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={navigate} />);
  expect(await screen.findByText("可恢复失败项：2 项")).toBeTruthy();
  expect(screen.getByText(/不会采用当前 Create 中尚未提交的修改/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "重试原运行的可恢复失败项" }));
  await waitFor(() => expect(api.retry).toHaveBeenCalledTimes(1));
  expect(api.retry).toHaveBeenCalledWith("project-a", { ref, selectedItemIds: ids });
  expect(useStudioStore.getState().values).toEqual(before); expect(useStudioStore.getState().pendingRunIntent).toBeUndefined();
  expect(navigate).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled(); expect(api.start).not.toHaveBeenCalled(); expect(api.resultReview).not.toHaveBeenCalled();
});
it("m2_mixed failure retains both typed recovery routes and already completed results", async () => {
  api.get.mockResolvedValue({ ...run, status: "PARTIAL", availableActions: ["RETRY", "EDIT_INPUT"], recoverability: { retryItemIds: ["item-1"], reviewRequired: 1 }, progress: { total: 3, succeeded: 1, failed: 2, cancelled: 0 } });
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={vi.fn()} />);
  expect(await screen.findByText(/部分失败项可以按原快照重试/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "重试原运行的可恢复失败项" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "编辑这些输入并创建新运行" })).toBeTruthy();
  expect(screen.getByText(/已完成的结果会保留/)).toBeTruthy(); expect(screen.getByText("视频结果")).toBeTruthy();
  expect(screen.getByText(/执行成功、素材存在、审核通过与镜头选用是四个独立事实/)).toBeTruthy();
  expect(api.retry).not.toHaveBeenCalled(); expect(api.resultReview).not.toHaveBeenCalled();
});
it.each([{ availableActions: [] }, { availableActions: ["RETRY"] }])("m2_empty retry ids never produce a fake retry, even with capability %j", async ({ availableActions }) => {
  api.get.mockResolvedValue({ ...run, availableActions, recoverability: { retryItemIds: [], reviewRequired: 0 }, errorSummary: "RETRY EDIT_INPUT START 自动恢复", detail: { ...run.detail, inputs: [{ ...run.detail!.inputs[0], errorMessage: "RETRY 连接失败" }] } });
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={vi.fn()} />);
  expect(await screen.findByText(/当前没有可自动重试的失败项。原因暂未分类/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "重试原运行的可恢复失败项" })).toBeNull();
  expect(screen.queryByText(/有失败项需要修改输入/)).toBeNull(); expect(screen.queryByRole("button", { name: "启动运行" })).toBeNull();
  expect(screen.getByText("视频结果")).toBeTruthy(); expect(screen.getByText(/已完成的结果会保留/)).toBeTruthy();
});
it("m2_paused start is continue, not retry or automatic start", async () => {
  api.get.mockResolvedValue({ ...run, status: "PAUSED", availableActions: ["START"], recoverability: { retryItemIds: [], reviewRequired: 0 }, progress: { total: 1, succeeded: 0, failed: 0, cancelled: 0 }, errorSummary: null });
  api.start.mockResolvedValue(run);
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={vi.fn()} />);
  expect(await screen.findByText(/运行已暂停，不代表已完成或失败/)).toBeTruthy();
  expect(api.start).not.toHaveBeenCalled(); expect(screen.queryByRole("button", { name: /重试原运行/ })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "继续运行" })); await waitFor(() => expect(api.start).toHaveBeenCalledWith("project-a", ref));
  expect(api.retry).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
});
it.each(["SUCCEEDED", "QUEUED", "RUNNING"] as const)("m2_%s does not force recovery warnings even with stale display-only error text", async status => {
  api.get.mockResolvedValue({ ...run, status, availableActions: [], recoverability: { retryItemIds: [], reviewRequired: 0 } });
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={vi.fn()} />);
  await screen.findByRole("button", { name: "编辑这些输入并创建新运行" });
  expect(screen.queryByRole("status", { name: "运行恢复说明" })).toBeNull(); expect(api.retry).not.toHaveBeenCalled();
});
it.each([false, true])("m2_cancelled retry requires backend capability: %s", async allowed => {
  api.get.mockResolvedValue({ ...run, status: "CANCELLED", availableActions: allowed ? ["RETRY"] : [], recoverability: { retryItemIds: ["item-1"], reviewRequired: 0 } });
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={vi.fn()} />);
  await screen.findByRole("button", { name: "编辑这些输入并创建新运行" });
  expect(Boolean(screen.queryByRole("button", { name: "重试原运行的可恢复失败项" }))).toBe(allowed);
});
it("m2_incomplete input is labelled and does not claim a complete snapshot", async () => {
  api.get.mockResolvedValue({ ...run, detail: { ...run.detail, inputs: [{ ...run.detail!.inputs[0], values: {}, reuseUnavailableReason: "部分历史输入无法安全恢复，将使用生成器默认值重新编辑。" }] } });
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={vi.fn()} />);
  expect(await screen.findByText(/部分历史输入无法安全恢复/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "编辑这些输入并创建新运行" }).hasAttribute("disabled")).toBe(false);
  expect(screen.getByText(/只有再次确认生成后/)).toBeTruthy();
});
it("m2_unavailable exact generator never switches to a recommended alternative", async () => {
  api.generatorsList.mockResolvedValue([{ ...generator, selectionRef: "different-pair" }]); const navigate = vi.fn();
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={navigate} />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑这些输入并创建新运行" }));
  expect((await screen.findByRole("alert")).textContent).toContain("生成器当前不可用");
  expect(navigate).not.toHaveBeenCalled(); expect(useStudioStore.getState().pendingRunIntent).toBeUndefined();
  expect(api.generate).not.toHaveBeenCalled(); expect(api.retry).not.toHaveBeenCalled();
});
it("m2_project switch rejects delayed A reuse completion without writing B's intent", async () => {
  let resolve!: (value: GeneratorOption[]) => void;
  const deferred = new Promise<GeneratorOption[]>(r => { resolve = r; });
  api.generatorsList.mockImplementation((_p, stage) => stage === "video" ? deferred : Promise.resolve([]));
  const navigate = vi.fn(); const view = render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={navigate} />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑这些输入并创建新运行" }));
  await waitFor(() => expect(api.generatorsList).toHaveBeenCalled());
  view.rerender(<RunsPage route={{ kind: "runs", projectId: "project-b", run: { source: "task", id: "b" } }} navigate={navigate} />);
  await act(async () => resolve([generator]));
  expect(navigate).not.toHaveBeenCalled(); expect(useStudioStore.getState().pendingRunIntent).toBeUndefined();
  expect(api.retry).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
});
it("m2_background refresh rereads recovery facts without choosing a recovery or overwriting draft", async () => {
  const navigate = vi.fn(); useStudioStore.getState().setValue("prompt", { type: "string", value: "keep draft" });
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={navigate} />);
  await screen.findByRole("button", { name: "编辑这些输入并创建新运行" }); const before = useStudioStore.getState().values;
  api.get.mockResolvedValue({ ...run, availableActions: ["RETRY"], recoverability: { retryItemIds: ["item-2"], reviewRequired: 0 } });
  invalidateRuns("project-a"); await screen.findByText("可恢复失败项：1 项");
  expect(useStudioStore.getState().values).toEqual(before); expect(useStudioStore.getState().pendingRunIntent).toBeUndefined();
  expect(navigate).not.toHaveBeenCalled(); expect(api.retry).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled(); expect(api.start).not.toHaveBeenCalled();
});

it("ordinary Runs exposes recent coverage and project-scoped existing complete task history",async()=>{
 const navigate=vi.fn();render(<RunsPage route={{kind:"runs",projectId:"project-a"}} navigate={navigate}/>);
 expect(screen.getByRole("heading",{name:"近期运行"})).toBeTruthy();expect(screen.getByText(/当前覆盖最近 50 个任务/)).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"查看完整任务历史"}));expect(navigate).toHaveBeenCalledWith({kind:"project-settings",projectId:"project-a",section:"advanced-tasks"});
});
