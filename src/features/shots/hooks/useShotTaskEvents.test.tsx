// @vitest-environment jsdom

import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TaskView } from "../../../types/task";
import { useShotTaskEvents, type ShotProductionModeTab } from "./useShotTaskEvents";

const mocks = vi.hoisted(() => ({
  subscribeTaskUpdates: vi.fn(),
  refreshQueues: vi.fn(),
  refreshMultiPackage: vi.fn(),
  refreshMonitor: vi.fn(),
}));

vi.mock("../../../services/taskEvents", () => ({ subscribeTaskUpdates: mocks.subscribeTaskUpdates }));

let listener: ((task: TaskView) => void) | undefined;
let unlisten: ReturnType<typeof vi.fn>;

function Harness({ tab = "package", enabled = true }: { tab?: ShotProductionModeTab; enabled?: boolean }) {
  useShotTaskEvents({
    enabled,
    projectId: "project-1",
    productionModeTab: tab,
    onRefreshQueues: mocks.refreshQueues,
    onRefreshMultiPackage: mocks.refreshMultiPackage,
    getMonitorBatchId: () => "batch-a",
    onRefreshMonitor: mocks.refreshMonitor,
  });
  return null;
}

function task(overrides: Partial<TaskView> = {}): TaskView {
  return {
    id: "task-1",
    projectId: "project-1",
    status: "SUCCEEDED",
    progress: { mode: "step", current: 1, total: 1 },
    createdAt: "2026-09-08T00:00:00Z",
    finishedAt: "2026-09-08T00:00:01Z",
    outputAssetIds: [],
    ...overrides,
  };
}

async function flush() {
  await act(async () => { await Promise.resolve(); });
}

beforeEach(() => {
  vi.useFakeTimers();
  listener = undefined;
  unlisten = vi.fn();
  mocks.subscribeTaskUpdates.mockImplementation(async (next: (value: TaskView) => void) => {
    listener = next;
    return unlisten;
  });
  mocks.refreshQueues = vi.fn();
  mocks.refreshMultiPackage = vi.fn();
  mocks.refreshMonitor = vi.fn();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.resetAllMocks();
});

describe("useShotTaskEvents", () => {
  it("filters project and non-terminal events, then debounces terminal refreshes", async () => {
    render(<Harness />);
    await flush();
    listener?.(task({ projectId: "other-project" }));
    listener?.(task({ status: "RUNNING" }));
    expect(mocks.refreshQueues).not.toHaveBeenCalled();

    listener?.(task());
    listener?.(task({ id: "task-2", status: "FAILED" }));
    await act(async () => { await vi.advanceTimersByTimeAsync(899); });
    expect(mocks.refreshQueues).not.toHaveBeenCalled();
    await act(async () => { await vi.advanceTimersByTimeAsync(1); });
    expect(mocks.refreshQueues).toHaveBeenCalledTimes(1);
    expect(mocks.refreshMonitor).toHaveBeenCalledWith("batch-a");
  });

  it("routes multi-package terminal refreshes and cleans up timer/listener", async () => {
    const view = render(<Harness tab="multi-package" />);
    await flush();
    listener?.(task());
    view.unmount();
    await act(async () => { await vi.advanceTimersByTimeAsync(900); });

    expect(mocks.refreshMultiPackage).not.toHaveBeenCalled();
    expect(mocks.refreshQueues).not.toHaveBeenCalled();
    expect(mocks.refreshMonitor).not.toHaveBeenCalled();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
