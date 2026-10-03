// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
// @ts-expect-error Node helpers are used only in Vitest.
import { readFileSync, readdirSync } from "node:fs";
// @ts-expect-error Node helper is used only in the architecture target.
import { execFileSync } from "node:child_process";
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
  fireEvent.click(await screen.findByRole("button", { name: "使用这些输入重新创作" }));
  await waitFor(() => expect(navigate).toHaveBeenCalledWith({ kind: "create", projectId: "project-a", stage: "video", shotId: "shot-a" }));
  expect(useStudioStore.getState().pendingRunIntent?.selectionRef).toBe("exact-pair"); expect(api.retry).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
  cleanup(); navigate.mockClear();
  api.get.mockResolvedValue({ ...run, detail: { ...run.detail, inputs: [{ ...run.detail!.inputs[0], values: {}, reuseUnavailableReason: "历史输入不完整，将使用默认值重新编辑。" }] } });
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: ref }} navigate={navigate} />);
  fireEvent.click(await screen.findByRole("button", { name: "使用这些输入重新创作" }));
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
  await screen.findByRole("button", { name: "使用这些输入重新创作" });
  expect(screen.getByText("高级技术详情").parentElement?.hasAttribute("open")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "进行中" })); expect(navigate).toHaveBeenCalledWith(expect.objectContaining({ filter: "active" }));
  api.get.mockResolvedValue({ ...run, status: "QUEUED", availableActions: ["START"] });
  invalidateRuns("project-a");
  api.start.mockRejectedValue({ code: "RUN_START_BLOCKED" });
  // Prove background refresh cannot erase the denied action's reason.
  fireEvent.click(await screen.findByRole("button", { name: "启动 / 重新启动" }));
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
  function Harness() { const [route, navigate] = useState<AppRoute>({ kind: "runs", projectId: "project-a", run: ref }); return route.kind === "create" ? <CreatePage route={route} navigate={navigate} /> : route.kind === "runs" ? <RunsPage route={route} navigate={navigate} /> : null; }
  render(<Harness />); fireEvent.click(await screen.findByRole("button", { name: "使用这些输入重新创作" }));
  await screen.findByRole("button", { name: "生成" }); expect(useStudioStore.getState().values.prompt).toEqual({ type: "string", value: "原始提示词" }); expect(useStudioStore.getState().values.duration_seconds).toEqual({ type: "integer", value: 5 }); expect(useStudioStore.getState().pendingRunIntent).toBeUndefined(); expect(api.generate).not.toHaveBeenCalled();
  const create = readFileSync("src/features/create/CreateController.ts", "utf8"); expect(create).toContain('kind: "runs"');
  cleanup(); useStudioStore.getState().resetDraft();
  api.creationGet.mockResolvedValue({ ...context, selectedShot: null });
  api.get.mockResolvedValue({ ...run, detail: { ...run.detail, sources: [] } });
  render(<Harness />); fireEvent.click(await screen.findByRole("button", { name: "使用这些输入重新创作" }));
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
  const output = execFileSync("node", ["scripts/dev088-architecture-guard.mjs"], { encoding: "utf8" }); expect(output).toContain("PRODUCT_FACADE_BOUNDARY=PASS"); expect(output).toContain("PRODUCT_COMMAND_PARITY=PASS");
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
