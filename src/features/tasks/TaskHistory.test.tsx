// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TaskDetail } from "../../types/history";
import { TaskHistory } from "./TaskHistory";

const mocks = vi.hoisted(() => ({
  getTaskDetail: vi.fn(),
  taskHistoryPage: vi.fn(),
  subscribeTaskUpdates: vi.fn(),
}));

vi.mock("../../services/tauriClient", () => ({
  getTaskDetail: mocks.getTaskDetail,
  taskHistoryPage: mocks.taskHistoryPage,
}));
vi.mock("../../services/taskEvents", () => ({ subscribeTaskUpdates: mocks.subscribeTaskUpdates }));
vi.mock("../assets/AssetPreview", () => ({ AssetPreview: () => null }));
vi.mock("../production/ProductionAuditCenter", () => ({ ProductionAuditCenter: () => null }));
vi.mock("./TaskHistoryDetail", () => ({
  TaskHistoryDetail: ({ detail }: { detail: TaskDetail }) => <output data-testid="focused-task">{detail.id}</output>,
}));
vi.mock("./TaskHistoryList", () => ({ TaskHistoryList: () => <div aria-label="任务列表" /> }));

afterEach(() => cleanup());

describe("TaskHistory exact focus", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.taskHistoryPage.mockResolvedValue({ items: [], nextCursor: undefined, workflowOptions: [] });
    mocks.subscribeTaskUpdates.mockResolvedValue(vi.fn());
    mocks.getTaskDetail.mockResolvedValue({ id: "task-99", outputAssets: [] } as never);
  });

  it("loads the requested task directly instead of assuming it is on the first page", async () => {
    render(
      <TaskHistory
        projectId="project-1"
        comfyConnected={false}
        productionBusy={false}
        focusTaskId="task-99"
        onLoadInputs={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByTestId("focused-task").textContent).toBe("task-99"));
    expect(mocks.getTaskDetail).toHaveBeenCalledWith("project-1", "task-99");
  });
});
