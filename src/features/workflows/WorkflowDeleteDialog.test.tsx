// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { WorkflowDeletionInspection, WorkflowProductionWorkspaceView } from "../../types/workflowOnboarding";
import { WorkflowDeleteDialog } from "./WorkflowDeleteDialog";

const item: WorkflowProductionWorkspaceView = {
  packageName: "builtin-package",
  builtin: true,
  archived: false,
  packageStatus: "VALID",
  workflowId: "WF1",
  workflowVersionId: "WV1",
  name: "系统工作流",
  workflowVersion: "1.0.0",
  enabled: true,
  capability: "READY",
  readiness: "READY",
  readinessReasons: [],
  capabilityIssues: [],
  nodeCount: 1,
  recipes: [],
  activeTasks: 0,
  totalTasks: 0,
  hasSuccessfulRun: false,
  diagnostics: [],
};

const inspection: WorkflowDeletionInspection = {
  workflowId: "WF1",
  workflowVersionId: "WV1",
  name: "系统工作流",
  builtin: true,
  enabled: true,
  archived: false,
  activeTaskCount: 0,
  activeQueueItemCount: 0,
  historicalTaskCount: 2,
  productionBatchItemCount: 1,
  benchmarkReferenceCount: 0,
  projectBindingCount: 1,
  canHardDelete: false,
  requiresArchive: true,
  blockingReasons: [],
  deleteAction: "REMOVE",
};

afterEach(() => cleanup());

describe("WorkflowDeleteDialog", () => {
  it("显示系统来源和删除影响，并使用确认回调", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();

    render(<WorkflowDeleteDialog item={item} inspection={inspection} onClose={vi.fn()} onConfirm={onConfirm} />);

    expect(screen.getByRole("heading", { name: "删除工作流" })).toBeTruthy();
    expect(screen.getByText("系统自带")).toBeTruthy();
    expect(screen.getByText("该工作流当前被 1 个项目配置使用。删除后将解除这些项目工作流配置。")).toBeTruthy();
    expect(screen.getByText(/已有生产记录仍然保留/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "删除工作流" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("活动任务时保留删除入口但解释阻塞原因并禁用确认", () => {
    render(
      <WorkflowDeleteDialog
        item={{ ...item, builtin: false }}
        inspection={{ ...inspection, builtin: false, activeTaskCount: 1, deleteAction: "BLOCKED", blockingReasons: ["有活动任务"] }}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );

    expect(screen.getByText(/当前有活动任务或队列项目，暂时不能删除/)).toBeTruthy();
    expect((screen.getByRole("button", { name: "删除工作流" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
