// @vitest-environment jsdom
import { cleanup, render, waitFor, act } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ProductionQueuePanel } from "./ProductionQueuePanel";
import { invalidateRuns, subscribeRunInvalidation } from "../../product/runInvalidation";

const api = vi.hoisted(() => ({
  list: vi.fn(),
  get: vi.fn(),
  overview: vi.fn(),
  start: vi.fn(),
}));

vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  listProductionQueues: api.list,
  getProductionQueue: api.get,
  getProductionQueueOverview: api.overview,
  listProductionQueueNamePresets: vi.fn().mockResolvedValue([]),
  startProductionQueue: api.start,
  getProductionAdmissionStatus: vi.fn().mockResolvedValue({ busy: false }),
}));
vi.mock("../../services/taskEvents", () => ({ subscribeTaskUpdates: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("./ProductionPartialResumePanel", () => ({ ProductionPartialResumePanel: () => null }));

afterEach(() => { cleanup(); vi.clearAllMocks(); });

const summary = (projectId: string, id: string, name: string) => ({
  id, projectId, name, status: "READY", total: 1, pending: 1, running: 0, succeeded: 0, failed: 0, cancelled: 0, skipped: 0, createdAt: "", updatedAt: "",
});
const overview = { totalQueues: 1, runningQueues: 0, pendingItems: 1, activeItems: 0, succeededItems: 0, failedItems: 0, archivedQueues: 0 };

it("rejects stale refreshQueues results after project switch", async () => {
  let resolveOld!: (v: unknown) => void;
  const oldList = new Promise((r) => { resolveOld = r; });
  api.list.mockImplementation((projectId: string) => {
    if (projectId === "project-a") return oldList;
    return Promise.resolve([summary("project-b", "b1", "B queue")]);
  });
  api.overview.mockImplementation((projectId: string) =>
    Promise.resolve({ ...overview, totalQueues: projectId === "project-b" ? 1 : 0 }),
  );
  api.get.mockResolvedValue({ ...summary("project-b", "b1", "B queue"), items: [] });

  const props = {
    batchItems: [] as [],
    comfyConnected: true,
    onAdmissionChanged: async () => {},
    onFocusedBatchOpened: () => {},
    onOpenTask: () => {},
  };
  const view = render(<ProductionQueuePanel projectId="project-a" {...props} />);
  await act(async () => {});
  view.rerender(<ProductionQueuePanel projectId="project-b" {...props} />);
  await waitFor(() => expect(api.list).toHaveBeenCalledWith("project-b"));
  await waitFor(() => expect(view.container.textContent).toContain("B queue"));
  await act(async () => { resolveOld([summary("project-a", "a1", "STALE A queue")]); });
  expect(view.container.textContent).toContain("B queue");
  expect(view.container.textContent).not.toContain("STALE A queue");
});

it("startQueue invalidates Runs without forcing Library thrash", async () => {
  const ready = {
    ...summary("p", "batch-1", "ready batch"),
    status: "READY",
    items: [{ id: "i1", ordinal: 0, status: "PENDING", workflowVersionId: "v", recipeId: "r", values: {} }],
  };
  api.list.mockResolvedValue([ready]);
  api.overview.mockResolvedValue(overview);
  api.get.mockResolvedValue(ready);
  api.start.mockResolvedValue({ ...ready, status: "RUNNING" });

  const seen: Array<{ projectId: string; library?: boolean }> = [];
  const stop = subscribeRunInvalidation((projectId, options) => {
    seen.push({ projectId, library: options?.library });
  });

  const { findByRole } = render(
    <ProductionQueuePanel
      projectId="p"
      batchItems={[]}
      comfyConnected
      focusBatchId="batch-1"
      onAdmissionChanged={async () => {}}
      onFocusedBatchOpened={() => {}}
      onOpenTask={() => {}}
    />,
  );
  const { default: userEvent } = await import("@testing-library/user-event");
  await userEvent.setup().click(await findByRole("button", { name: "开始生产" }));
  await waitFor(() => expect(api.start).toHaveBeenCalledWith("p", "batch-1"));
  await waitFor(() => expect(seen.some((e) => e.projectId === "p" && e.library === false)).toBe(true));
  stop();
});

it("invalidateRuns library:false is the cheap generate/start contract", () => {
  const hits: Array<boolean | undefined> = [];
  const stop = subscribeRunInvalidation((_id, options) => hits.push(options?.library));
  invalidateRuns("x", { library: false });
  invalidateRuns("x");
  stop();
  expect(hits).toEqual([false, undefined]);
});
