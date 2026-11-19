// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ProductionQueuePanel } from "./ProductionQueuePanel";

const api = vi.hoisted(() => ({ list: vi.fn(), get: vi.fn(), overview: vi.fn(), start: vi.fn(), pause: vi.fn(), cancel: vi.fn(), requeue: vi.fn(), presets: vi.fn(), subscribe: vi.fn() }));
vi.mock("../../services/tauriClient", async importOriginal => ({
  ...await importOriginal<typeof import("../../services/tauriClient")>(),
  listProductionQueues: api.list, getProductionQueue: api.get, getProductionQueueOverview: api.overview,
  listProductionQueueNamePresets: api.presets,
  requeueProductionQueueItem: api.requeue,
  startProductionQueue: api.start, pauseProductionQueue: api.pause, cancelPendingProductionQueue: api.cancel,
}));
vi.mock("../../services/taskEvents", () => ({ subscribeTaskUpdates: api.subscribe }));
vi.mock("./ProductionPartialResumePanel", () => ({ ProductionPartialResumePanel: () => null }));
const running = { id: "mixed", projectId: "p", name: "mixed four modes", status: "RUNNING", total: 2, pending: 1, running: 1, succeeded: 0, failed: 0, cancelled: 0, skipped: 0, createdAt: "", updatedAt: "", items: [
  { id: "a", ordinal: 0, status: "RUNNING", workflowVersionId: "version", recipeId: "t2v", taskId: "task-a", values: {} },
  { id: "b", ordinal: 1, status: "PENDING", workflowVersionId: "version2", recipeId: "first-last", values: {} },
] };
beforeEach(() => { api.presets.mockResolvedValue([]); api.subscribe.mockResolvedValue(() => {}); });
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

const failedQueue = {...running,status:"PAUSED",running:0,pending:0,failed:2,items:running.items.map(item=>({...item,status:"FAILED",errorCode:"COMFY_TIMEOUT"}))};
it.each(["single","batch"])("R52: %s replay of an existing retry does not claim a new child",async kind=>{
 const existing={...failedQueue,total:3,pending:1,items:[...failedQueue.items,{...running.items[1],id:"existing-child",ordinal:2,retryOfItemId:"a",promptText:"原有重试"}]};
 api.list.mockResolvedValue([existing]);api.get.mockResolvedValue(existing);api.overview.mockResolvedValue({});api.requeue.mockResolvedValue(existing);
 const user=userEvent.setup();render(<ProductionQueuePanel projectId="p" batchItems={[]} comfyConnected focusBatchId="mixed" hideCreate onAdmissionChanged={async()=>{}} onFocusedBatchOpened={()=>{}} onOpenTask={()=>{}}/>);
 await screen.findByText("原有重试");await user.click(kind==="single"?screen.getAllByRole("button",{name:"创建重试项"})[0]:screen.getByRole("button",{name:"批量创建安全重试项"}));
 expect(await screen.findByText(kind==="single"?/本次未新增/:/实际新增 0 个待执行项/)).toBeTruthy();expect(api.start).not.toHaveBeenCalled();
});
it("R53: a later failed retry preserves the first child and reports partial success",async()=>{
 const child={...running.items[1],id:"new-child",ordinal:2,retryOfItemId:"a",promptText:"保留成功重试项"};
 const updated={...failedQueue,total:3,pending:1,items:[...failedQueue.items,child]};
 api.list.mockResolvedValue([failedQueue]);api.get.mockResolvedValue(failedQueue);api.overview.mockResolvedValue({});api.requeue.mockResolvedValueOnce(updated).mockRejectedValueOnce(new Error("controlled later failure"));
 const user=userEvent.setup();render(<ProductionQueuePanel projectId="p" batchItems={[]} comfyConnected focusBatchId="mixed" hideCreate onAdmissionChanged={async()=>{}} onFocusedBatchOpened={()=>{}} onOpenTask={()=>{}}/>);
 await screen.findByRole("button",{name:"批量创建安全重试项"});await user.click(screen.getByRole("button",{name:"批量创建安全重试项"}));
 expect(await screen.findByText(/已处理 1 项，实际新增 1 个待执行重试项/)).toBeTruthy();expect(await screen.findByText("保留成功重试项")).toBeTruthy();expect(api.requeue).toHaveBeenCalledTimes(2);expect(api.start).not.toHaveBeenCalled();
});
