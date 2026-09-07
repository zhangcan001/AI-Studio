// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProductionBatchDetail, ProductionBatchSummary, ProductionQueueOverview } from "../../../types/productionQueue";
import { useShotQueueController } from "./useShotQueueController";

const mocks = vi.hoisted(() => ({
  getProductionAdmissionStatus: vi.fn(),
  getProductionQueue: vi.fn(),
  getProductionQueueOverview: vi.fn(),
  listProductionQueues: vi.fn(),
  pauseProductionQueue: vi.fn(),
  requeueProductionQueueItem: vi.fn(),
  startProductionQueue: vi.fn(),
  reloadWorkspace: vi.fn(),
  refreshMonitor: vi.fn(),
  onError: vi.fn(),
  onNotice: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return {
    ...actual,
    getProductionAdmissionStatus: mocks.getProductionAdmissionStatus,
    getProductionQueue: mocks.getProductionQueue,
    getProductionQueueOverview: mocks.getProductionQueueOverview,
    listProductionQueues: mocks.listProductionQueues,
    pauseProductionQueue: mocks.pauseProductionQueue,
    requeueProductionQueueItem: mocks.requeueProductionQueueItem,
    startProductionQueue: mocks.startProductionQueue,
  };
});

const queues: ProductionBatchSummary[] = [
  {
    id: "batch-a",
    projectId: "project-1",
    name: "Batch A",
    status: "READY",
    continueOnFailure: false,
    createdAt: "2026-09-08T00:00:00Z",
    updatedAt: "2026-09-08T00:00:00Z",
  },
  {
    id: "batch-b",
    projectId: "project-1",
    name: "Batch B",
    status: "READY",
    continueOnFailure: false,
    createdAt: "2026-09-08T00:00:01Z",
    updatedAt: "2026-09-08T00:00:01Z",
  },
];

const overview: ProductionQueueOverview = {
  totalQueues: 2,
  runningQueues: 0,
  pausedQueues: 0,
  completedQueues: 0,
  archivedQueues: 0,
  totalItems: 2,
  pendingItems: 2,
  activeItems: 0,
  succeededItems: 0,
  failedItems: 0,
  cancelledItems: 0,
  skippedItems: 0,
};

function detail(id: string): ProductionBatchDetail {
  return {
    ...queues.find((queue) => queue.id === id)!,
    total: 1,
    pending: 0,
    running: 0,
    succeeded: 1,
    failed: 0,
    cancelled: 0,
    skipped: 0,
    status: "COMPLETED",
    items: [],
  };
}

function Harness({ projectId = "project-1" }: { projectId?: string }) {
  const controller = useShotQueueController({
    projectId,
    enabled: true,
    reloadWorkspace: mocks.reloadWorkspace,
    refreshProductionMonitor: mocks.refreshMonitor,
    onError: mocks.onError,
    onNotice: mocks.onNotice,
  });
  return (
    <div>
      <output data-testid="status">{controller.sequential.status}</output>
      <output data-testid="current">{controller.sequential.currentBatchId ?? ""}</output>
      <output data-testid="queued">{controller.sequential.queuedBatchIds.join(",")}</output>
      <button type="button" onClick={() => void controller.startBatch("batch-a")}>Start A</button>
      <button type="button" onClick={() => void controller.startBatch("batch-b")}>Start B</button>
    </div>
  );
}

async function flush() {
  await act(async () => { await Promise.resolve(); await Promise.resolve(); });
}

beforeEach(() => {
  mocks.listProductionQueues.mockResolvedValue(queues);
  mocks.getProductionQueueOverview.mockResolvedValue(overview);
  mocks.getProductionAdmissionStatus.mockResolvedValue({ busy: false });
  mocks.getProductionQueue.mockImplementation(async (_projectId: string, id: string) => detail(id));
  mocks.startProductionQueue.mockResolvedValue(undefined);
  mocks.pauseProductionQueue.mockResolvedValue(undefined);
  mocks.requeueProductionQueueItem.mockResolvedValue(undefined);
  mocks.reloadWorkspace.mockResolvedValue(undefined);
  mocks.refreshMonitor.mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("useShotQueueController", () => {
  it("starts a free batch and queues a second explicit start", async () => {
    render(<Harness />);
    await flush();
    fireEvent.click(screen.getByRole("button", { name: "Start A" }));
    await flush();
    expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-1", "batch-a");
    fireEvent.click(screen.getByRole("button", { name: "Start B" }));
    expect(screen.getByTestId("current").textContent).toBe("batch-a");
    expect(screen.getByTestId("queued").textContent).toBe("batch-b");
  });

  it("resets the sequential session when the project changes", async () => {
    const view = render(<Harness projectId="project-1" />);
    await flush();
    fireEvent.click(screen.getByRole("button", { name: "Start A" }));
    await flush();
    expect(screen.getByTestId("current").textContent).toBe("batch-a");
    view.rerender(<Harness projectId="project-2" />);
    await flush();
    expect(screen.getByTestId("status").textContent).toBe("IDLE");
    expect(screen.getByTestId("current").textContent).toBe("");
    expect(screen.getByTestId("queued").textContent).toBe("");
  });
});
