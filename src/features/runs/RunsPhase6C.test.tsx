// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { RunsPage } from "./RunsPage";
import { actionLabel, coverageMessage, runKey, sourceLabels } from "./runsModel";
import type { ProductRun, RunResult } from "../../product/types";

const api = vi.hoisted(() => ({
  list: vi.fn(), get: vi.fn(), resultsGet: vi.fn(), start: vi.fn(), pause: vi.fn(),
  cancel: vi.fn(), retry: vi.fn(), resultReview: vi.fn(), generatorsList: vi.fn(),
}));
vi.mock("../../product/client", () => ({
  productClient: {
    run: api,
    creation: { generatorsList: (...a: unknown[]) => api.generatorsList(...a), mediaUrl: () => "http://fixture/media" },
  },
}));
vi.mock("../../product/runInvalidation", () => ({
  invalidateRuns: vi.fn(),
  subscribeRunInvalidation: () => () => undefined,
}));

const task: ProductRun = {
  ref: { source: "task", id: "task-a" }, projectId: "project-a", title: "任务 A", status: "SUCCEEDED",
  phase: "SUCCEEDED", createdAt: "2026-10-02T00:00:00Z", updatedAt: "2026-10-02T00:00:00Z",
  progress: { total: 1, succeeded: 1, failed: 0, cancelled: 0 }, recoverability: { retryItemIds: [], reviewRequired: 0 },
  resultsSummary: ["ast-1"], errorSummary: null, preferredParent: null,
  availableActions: [], detail: { sources: [], inputs: [] },
};
const batch: ProductRun = {
  ...task, ref: { source: "queue-batch", id: "batch-a" }, title: "批次 A", status: "RUNNING",
  availableActions: ["PAUSE", "CANCEL"], progress: { total: 2, succeeded: 0, failed: 0, cancelled: 0 }, resultsSummary: [],
};
const production: ProductRun = {
  ...task, ref: { source: "production-run", id: "prod-a" }, title: "生产 A", status: "QUEUED",
  availableActions: ["START"], resultsSummary: [],
};
const result: RunResult = {
  assetId: "ast-1", name: "结果", mediaKind: "video", assetExists: true, availability: "available",
  reviewState: "PENDING", reviewRevision: 1, selectedShotIds: [], thumbnailBytes: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  api.list.mockResolvedValue({
    items: [production, batch, task],
    nextCursor: null,
    coverage: "RECENT_50_TASKS_AND_PRODUCTION_WITH_UNARCHIVED_QUEUES",
  });
  api.get.mockImplementation(async (_p: string, ref: { source: string; id: string }) =>
    [production, batch, task].find(r => r.ref.source === ref.source && r.ref.id === ref.id) ?? task);
  api.resultsGet.mockResolvedValue([result]);
  api.start.mockImplementation(async (_p, ref) => ({ ...production, status: "RUNNING", ref, availableActions: ["PAUSE", "CANCEL"] }));
  api.pause.mockImplementation(async (_p, ref) => ({ ...batch, status: "PAUSED", ref, availableActions: ["START"] }));
  api.cancel.mockImplementation(async (_p, ref) => ({ ...batch, status: "CANCELLED", ref, availableActions: [] }));
  api.resultReview.mockResolvedValue(undefined);
});
afterEach(() => cleanup());

it("C01 RunSource labels and runKey distinguish production/queue/task without using titles as ids", () => {
  expect(runKey(production.ref)).toBe("production-run:prod-a");
  expect(runKey(batch.ref)).toBe("queue-batch:batch-a");
  expect(runKey(task.ref)).toBe("task:task-a");
  expect(sourceLabels["production-run"]).toBe("生产运行");
  expect(sourceLabels["queue-batch"]).toBe("队列批次");
  expect(sourceLabels.task).toBe("历史任务");
  expect(coverageMessage("RECENT_50_TASKS_AND_PRODUCTION_WITH_UNARCHIVED_QUEUES")).toContain("最近 50");
  expect(coverageMessage("RECENT_50_TASKS_AND_PRODUCTION_WITH_UNARCHIVED_QUEUES")).toContain("不提供完整混合运行历史分页");
});

it("C02/C03 Drawer-equivalent Runs actions: list shows shared statuses; start is explicit click only", async () => {
  const navigate = vi.fn();
  render(<RunsPage route={{ kind: "runs", projectId: "project-a" }} navigate={navigate} />);
  await screen.findByText("生产 A");
  expect(screen.getAllByText("生产运行").length).toBeGreaterThan(0);
  expect(screen.getAllByText("队列批次").length).toBeGreaterThan(0);
  expect(screen.getAllByText("历史任务").length).toBeGreaterThan(0);
  expect(screen.getAllByText("排队中").length).toBeGreaterThan(0);
  expect(screen.getAllByText("运行中").length).toBeGreaterThan(0);
  expect(screen.getByLabelText("运行列表").textContent).toContain("已完成");
  fireEvent.click(screen.getByRole("button", { name: /生产 A/ }));
  expect(navigate).toHaveBeenCalledWith(expect.objectContaining({ run: production.ref }));
  expect(api.start).not.toHaveBeenCalled();
  cleanup();
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: production.ref }} navigate={navigate} />);
  fireEvent.click(await screen.findByRole("button", { name: "启动运行" }));
  await waitFor(() => expect(api.start).toHaveBeenCalledWith("project-a", production.ref));
});

it("C04 pause/cancel action labels encode non-preemptive semantics", () => {
  expect(actionLabel("PAUSE", "RUNNING")).toContain("不抢占");
  expect(actionLabel("CANCEL", "RUNNING")).toContain("不中断当前推理");
});

it("C06 project fence: switching project clears prior list via new list call", async () => {
  const navigate = vi.fn();
  const view = render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: task.ref }} navigate={navigate} />);
  await screen.findByLabelText("运行详情");
  expect(screen.getByLabelText("运行详情").textContent).toContain("任务 A");
  api.list.mockResolvedValue({ items: [], nextCursor: null, coverage: "RECENT_50_TASKS_AND_PRODUCTION_WITH_UNARCHIVED_QUEUES" });
  view.rerender(<RunsPage route={{ kind: "runs", projectId: "project-b" }} navigate={navigate} />);
  await waitFor(() => expect(api.list).toHaveBeenCalledWith("project-b", "all", null));
  await screen.findByText("当前筛选没有运行。");
});

it("C07 success/select/review remain independent facts on results", async () => {
  render(<RunsPage route={{ kind: "runs", projectId: "project-a", run: task.ref }} navigate={vi.fn()} />);
  await screen.findByText(/执行成功、素材存在、审核通过与镜头选用是四个独立事实/);
  expect(screen.getByText(/镜头未选用/)).toBeTruthy();
  expect(screen.getByText(/待审核/)).toBeTruthy();
  fireEvent.click(await screen.findByRole("button", { name: "审核通过" }));
  await waitFor(() => expect(api.resultReview).toHaveBeenCalled());
  expect(api.resultReview.mock.calls[0][1]).toMatchObject({ decision: "APPROVED", assetId: "ast-1" });
});

it("C09 deep-link context restores RunRef and preserves review independence copy", async () => {
  render(<RunsPage route={{
    kind: "runs", projectId: "project-a", run: batch.ref, filter: "production",
    context: { shotId: "shot-a", batchId: "batch-a", reviewId: "rev-1" },
  }} navigate={vi.fn()} />);
  await screen.findByLabelText("历史运行定位");
  expect(screen.getByText(/审核仍需显式操作/)).toBeTruthy();
  const detail = await screen.findByLabelText("运行详情");
  expect(detail.textContent).toContain("队列批次");
  expect(screen.getByRole("button", { name: "暂停（不抢占当前推理）" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "取消未开始项（不中断当前推理）" })).toBeTruthy();
  expect(screen.getByText(/不提供完整混合运行历史分页/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: /下一页|更多历史/ })).toBeNull();
});

it("C05/C08 list never invents cursor; unexpected nextCursor is stripped and alerted", async () => {
  api.list.mockResolvedValueOnce({
    items: [task],
    nextCursor: "forged",
    coverage: "RECENT_50_TASKS_AND_PRODUCTION_WITH_UNARCHIVED_QUEUES",
  });
  render(<RunsPage route={{ kind: "runs", projectId: "project-a" }} navigate={vi.fn()} />);
  await screen.findByText(/服务端返回了意外游标/);
  expect(api.list).toHaveBeenCalledWith("project-a", "all", null);
});
