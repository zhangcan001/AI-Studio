// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProductionBatchReviewProductivity } from "../../../services/tauriClient";
import type { ProductionBatchDetail } from "../../../types/productionQueue";
import { useShotProductionMonitor } from "./useShotProductionMonitor";

const mocks = vi.hoisted(() => ({
  getProductionQueue: vi.fn(),
  getProductionBatchReviewProductivity: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return {
    ...actual,
    getProductionQueue: mocks.getProductionQueue,
    getProductionBatchReviewProductivity: mocks.getProductionBatchReviewProductivity,
  };
});

function detail(batchId: string, status: ProductionBatchDetail["status"] = "RUNNING"): ProductionBatchDetail {
  return {
    id: batchId,
    projectId: "project-1",
    name: batchId,
    status,
    continueOnFailure: false,
    createdAt: "2026-09-09T00:00:00Z",
    updatedAt: "2026-09-09T00:00:00Z",
    total: 1,
    pending: status === "RUNNING" ? 1 : 0,
    running: status === "RUNNING" ? 1 : 0,
    succeeded: status === "COMPLETED" ? 1 : 0,
    failed: 0,
    cancelled: 0,
    skipped: 0,
    items: [],
  };
}

function review(batchId: string, status: ProductionBatchDetail["status"] = "RUNNING") {
  return { batch: detail(batchId, status), total: 1, successCount: status === "COMPLETED" ? 1 : 0, failedCount: 0, unreviewedCount: 0, approvedCount: 0, starredCount: 0, regenerateCount: 0, rejectedCount: 0, items: [] } as ProductionBatchReviewProductivity;
}

function Harness({ enabled = true, selectedBatchId = "batch-a" }: { enabled?: boolean; selectedBatchId?: string }) {
  const controller = useShotProductionMonitor({ projectId: "project-1", enabled, selectedBatchId });
  return (
    <div>
      <output data-testid="batch">{controller.batch?.id ?? ""}</output>
      <output data-testid="loading">{String(controller.loading)}</output>
      <output data-testid="error">{controller.error ?? ""}</output>
      <button type="button" onClick={() => void controller.refresh(selectedBatchId)}>Refresh</button>
      <button type="button" onClick={() => controller.focusBatch("batch-focus")}>Focus</button>
    </div>
  );
}

async function flush() {
  await act(async () => { await Promise.resolve(); await Promise.resolve(); });
}

beforeEach(() => {
  Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
  mocks.getProductionQueue.mockImplementation(async (_projectId: string, batchId: string) => detail(batchId));
  mocks.getProductionBatchReviewProductivity.mockImplementation(async (_projectId: string, batchId: string) => review(batchId));
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.resetAllMocks();
});

describe("useShotProductionMonitor", () => {
  it("loads the selected batch and review once", async () => {
    render(<Harness />);
    await flush();
    expect(mocks.getProductionQueue).toHaveBeenCalledWith("project-1", "batch-a");
    expect(mocks.getProductionBatchReviewProductivity).toHaveBeenCalledWith("project-1", "batch-a");
    expect(screen.getByTestId("batch").textContent).toBe("batch-a");
  });

  it("does not request while initially hidden, then refreshes when visible", async () => {
    Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
    render(<Harness />);
    await flush();
    expect(mocks.getProductionQueue).not.toHaveBeenCalled();
    Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
    document.dispatchEvent(new Event("visibilitychange"));
    await flush();
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(1);
  });

  it("ignores a stale result after the selected batch changes", async () => {
    let resolveA: ((value: ProductionBatchDetail) => void) | undefined;
    let resolveB: ((value: ProductionBatchDetail) => void) | undefined;
    mocks.getProductionQueue.mockImplementation((_projectId: string, batchId: string) => new Promise((resolve) => {
      if (batchId === "batch-a") resolveA = resolve as (value: ProductionBatchDetail) => void;
      else resolveB = resolve as (value: ProductionBatchDetail) => void;
    }));
    mocks.getProductionBatchReviewProductivity.mockImplementation(async (_projectId: string, batchId: string) => review(batchId));
    const view = render(<Harness selectedBatchId="batch-a" />);
    await flush();
    view.rerender(<Harness selectedBatchId="batch-b" />);
    await flush();
    await act(async () => {
      resolveA?.(detail("batch-a"));
      await Promise.resolve();
      await Promise.resolve();
    });
    await act(async () => {
      resolveB?.(detail("batch-b"));
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.getByTestId("batch").textContent).toBe("batch-b");
  });

  it("polls every 3000ms while active and stops after terminal data", async () => {
    vi.useFakeTimers();
    let status: ProductionBatchDetail["status"] = "RUNNING";
    mocks.getProductionQueue.mockImplementation(async (_projectId: string, batchId: string) => detail(batchId, status));
    mocks.getProductionBatchReviewProductivity.mockImplementation(async (_projectId: string, batchId: string) => review(batchId, status));
    render(<Harness />);
    await flush();
    await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(2);
    status = "COMPLETED";
    await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
    await flush();
    const terminalCount = mocks.getProductionQueue.mock.calls.length;
    await act(async () => { await vi.advanceTimersByTimeAsync(6000); });
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(terminalCount);
  });

  it("coalesces refreshes and does not create an unbounded request chain", async () => {
    let release: (() => void) | undefined;
    mocks.getProductionQueue.mockImplementationOnce(() => new Promise<ProductionBatchDetail>((resolve) => {
      release = () => resolve(detail("batch-a"));
    }));
    render(<Harness />);
    await flush();
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(1);
    await act(async () => {
      release?.();
      await Promise.resolve();
      await Promise.resolve();
    });
    await flush();
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(2);
  });

  it("does not update state after unmount while a request is pending", async () => {
    let release: (() => void) | undefined;
    mocks.getProductionQueue.mockImplementation(() => new Promise<ProductionBatchDetail>((resolve) => {
      release = () => resolve(detail("batch-a"));
    }));
    const view = render(<Harness />);
    await flush();
    view.unmount();
    await act(async () => {
      release?.();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.getProductionQueue).toHaveBeenCalledTimes(1);
  });
});
