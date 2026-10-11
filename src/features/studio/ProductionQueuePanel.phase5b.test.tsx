// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { ProductionQueuePanel } from "./ProductionQueuePanel";

const api = vi.hoisted(() => ({ list: vi.fn(), get: vi.fn(), overview: vi.fn(), start: vi.fn(), pause: vi.fn(), cancel: vi.fn() }));
vi.mock("../../services/tauriClient", async importOriginal => ({
  ...await importOriginal<typeof import("../../services/tauriClient")>(),
  listProductionQueues: api.list, getProductionQueue: api.get, getProductionQueueOverview: api.overview,
  listProductionQueueNamePresets: vi.fn().mockResolvedValue([]),
  startProductionQueue: api.start, pauseProductionQueue: api.pause, cancelPendingProductionQueue: api.cancel,
}));
vi.mock("../../services/taskEvents", () => ({ subscribeTaskUpdates: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("./ProductionPartialResumePanel", () => ({ ProductionPartialResumePanel: () => null }));
const running = { id: "mixed", projectId: "p", name: "mixed four modes", status: "RUNNING", total: 2, pending: 1, running: 1, succeeded: 0, failed: 0, cancelled: 0, skipped: 0, createdAt: "", updatedAt: "", items: [
  { id: "a", ordinal: 0, status: "RUNNING", workflowVersionId: "version", recipeId: "t2v", taskId: "task-a", values: {} },
  { id: "b", ordinal: 1, status: "PENDING", workflowVersionId: "version2", recipeId: "first-last", values: {} },
] };
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.clearAllMocks(); });

it("keeps explicit Pause and Cancel Pending nonpreemptive; no Start on navigation or pause", async () => {
  api.list.mockResolvedValue([running]); api.get.mockResolvedValue(running); api.overview.mockResolvedValue({});
  api.pause.mockResolvedValue({ ...running, status: "PAUSED" });
  api.cancel.mockResolvedValue({ ...running, status: "PAUSED", pending: 0, cancelled: 1, items: [running.items[0], { ...running.items[1], status: "CANCELLED" }] });
  const user = userEvent.setup();
  render(<ProductionQueuePanel projectId="p" batchItems={[]} comfyConnected focusBatchId="mixed" hideCreate onAdmissionChanged={async () => {}} onFocusedBatchOpened={() => {}} onOpenTask={() => {}} />);
  await screen.findByText(/暂停仅停止后续派发/);
  expect(api.start).not.toHaveBeenCalled(); expect(api.pause).not.toHaveBeenCalled(); expect(api.cancel).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "暂停" }));
  await waitFor(() => expect(api.pause).toHaveBeenCalledWith("p", "mixed"));
  await screen.findByText(/队列已暂停，已经提交的任务会继续运行到结束/);
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  await user.click(screen.getByRole("button", { name: "取消剩余项" })); expect(api.cancel).not.toHaveBeenCalled();
  confirm.mockReturnValue(true); await user.click(screen.getByRole("button", { name: "取消剩余项" }));
  await waitFor(() => expect(api.cancel).toHaveBeenCalledWith("p", "mixed"));
  expect(confirm.mock.calls[confirm.mock.calls.length - 1]?.[0]).toContain("当前已派发任务会继续运行到结束"); expect(api.start).not.toHaveBeenCalled();
});
