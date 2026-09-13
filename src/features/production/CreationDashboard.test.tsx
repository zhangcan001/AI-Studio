// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CreationDashboard } from "./CreationDashboard";

const mocks = vi.hoisted(() => ({
  getProductionQueueOverview: vi.fn(),
  listProductionQueues: vi.fn(),
  listPromptLibrary: vi.fn(),
  pauseProductionQueue: vi.fn(),
  startProductionQueue: vi.fn(),
  taskHistoryPage: vi.fn(),
}));

vi.mock("../../services/tauriClient", () => mocks);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("CreationDashboard production language", () => {
  it("keeps READY as waiting and reports explicit start feedback after the queue runs", async () => {
    const user = userEvent.setup();
    let queueStatus = "READY";
    const queue = {
      id: "batch-1",
      projectId: "project-1",
      name: "第一批",
      status: queueStatus,
      continueOnFailure: false,
      createdAt: "2026-09-12T00:00:00Z",
      updatedAt: "2026-09-12T00:00:00Z",
    };
    mocks.listProductionQueues.mockImplementation(async () => [{ ...queue, status: queueStatus }]);
    mocks.getProductionQueueOverview.mockResolvedValue({
      totalQueues: 1,
      runningQueues: queueStatus === "RUNNING" ? 1 : 0,
      pausedQueues: 0,
      completedQueues: 0,
      archivedQueues: 0,
      totalItems: 1,
      pendingItems: 1,
      activeItems: queueStatus === "RUNNING" ? 1 : 0,
      succeededItems: 0,
      failedItems: 0,
      cancelledItems: 0,
      skippedItems: 0,
    });
    mocks.taskHistoryPage.mockResolvedValue({ items: [] });
    mocks.listPromptLibrary.mockResolvedValue({ items: [] });
    mocks.startProductionQueue.mockImplementation(async () => {
      queueStatus = "RUNNING";
      return { ...queue, status: queueStatus };
    });

    render(
      <CreationDashboard
        projectId="project-1"
        catalog={[]}
        promptTargetFieldKey=""
        onPromptTargetFieldChange={vi.fn()}
        onUsePrompt={vi.fn()}
        onContinueWorkflow={vi.fn()}
        onFocusQueue={vi.fn()}
        onAdmissionChanged={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    await waitFor(() => expect(mocks.listProductionQueues).toHaveBeenCalledWith("project-1"));
    const dashboard = document.querySelector(".creation-dashboard");
    const summary = dashboard?.querySelector("summary");
    if (!(summary instanceof HTMLElement)) throw new Error("missing dashboard summary");
    await user.click(summary);

    expect(screen.getByText(/待启动/)).toBeTruthy();
    const start = screen.getByRole("button", { name: "开始生产" });
    await user.click(start);

    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledWith("project-1", "batch-1"));
    expect((await screen.findByRole("status")).textContent).toContain("生产已启动，任务正在运行");
    expect(screen.getByRole("button", { name: "暂停" })).toBeTruthy();
  });
});
