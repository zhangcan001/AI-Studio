// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { WorkflowRecipeHistoryView } from "../../types/workflowHistory";
import { RecipeHistoryPane } from "./RecipeHistoryPane";

afterEach(cleanup);

function history(overrides: Partial<WorkflowRecipeHistoryView> = {}): WorkflowRecipeHistoryView {
  return {
    workflowId: "workflow-1",
    workflowVersionId: "version-1",
    recipeId: "recipe-1",
    workflowVersion: "1.0.0",
    recipeVersion: "2.0.0",
    schemaVersion: 1,
    recipeSha256: "sha-1",
    createdAt: "2026-09-10T00:00:00Z",
    isCurrentVersion: false,
    isPromoted: true,
    archived: true,
    workflowLibraryState: "REMOVED",
    taskCount: 1,
    activeTaskCount: 0,
    succeededTaskCount: 1,
    failedTaskCount: 0,
    cancelledTaskCount: 0,
    executedProjectCount: 1,
    referencedProjectCount: 2,
    presetCount: 0,
    projectTemplateCount: 0,
    productionRunTemplateCount: 0,
    queueItemCount: 1,
    shotStageCount: 0,
    experimentCount: 0,
    benchmarkRunCount: 0,
    projectBindingCount: 1,
    taskPage: { items: [{
      id: "task-1", projectId: "project-1", projectName: "Demo", status: "SUCCEEDED",
      createdAt: "2026-09-10T01:00:00Z", finishedAt: "2026-09-10T01:01:00Z",
    }], nextCursor: { createdAt: "2026-09-10T01:00:00Z", id: "task-1" } },
    queueItems: [{
      batchId: "batch-1", batchName: "Planned", projectId: "project-2", projectName: "Other",
      batchStatus: "READY", itemId: "item-1", itemStatus: "PENDING", taskId: null,
      createdAt: "2026-09-10T02:00:00Z", updatedAt: "2026-09-10T02:00:00Z",
    }],
    presets: [], projectTemplates: [], productionRunTemplates: [], projectBindings: [], shots: [], experiments: [],
    ...overrides,
  };
}

describe("RecipeHistoryPane", () => {
  it("renders exact technical identity and independent lifecycle/read-only usage facts", () => {
    render(<RecipeHistoryPane history={history()} loading={false} onClose={vi.fn()} onLoadMore={vi.fn()} />);

    expect(screen.getByText("工作流版本 1.0.0 · 已归档 · 已推广")).toBeTruthy();
    expect(screen.getByText("version-1")).toBeTruthy();
    expect(screen.getByText("recipe-1")).toBeTruthy();
    expect(screen.getByText("sha-1")).toBeTruthy();
    expect(screen.getByText("执行项目数").parentElement?.textContent).toContain("1");
    expect(screen.getByText("关联项目数").parentElement?.textContent).toContain("2");
    expect(screen.queryByRole("button", { name: /归档|恢复|推广|生成|重试|取消/ })).toBeNull();
  });

  it("keeps planned queue usage separate and supports bounded task pagination", async () => {
    const onLoadMore = vi.fn();
    const onOpenTask = vi.fn();
    render(<RecipeHistoryPane history={history()} loading={false} onClose={vi.fn()} onLoadMore={onLoadMore} onOpenTask={onOpenTask} />);

    expect(screen.getByText(/计划使用/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "查看任务" }));
    expect(onOpenTask).toHaveBeenCalledWith("task-1");
    await userEvent.click(screen.getByRole("button", { name: "读取更多任务" }));
    expect(onLoadMore).toHaveBeenCalledTimes(1);
  });
});
