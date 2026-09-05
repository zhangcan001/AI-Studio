// @vitest-environment jsdom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type { WorkflowPurgeInspection, WorkflowPurgeResult } from "../../types/workflowOnboarding";
import { useWorkflowOnboardingStore } from "../../stores/workflowOnboardingStore";
import { useWorkflowWorkspaceStore } from "../../stores/workflowWorkspaceStore";
import type { WorkflowWorkspaceQueryResponse } from "./workflowWorkspaceAdapters";
import { WorkflowWorkspace } from "./WorkflowWorkspace";

const workflowMocks = vi.hoisted(() => ({
  queryWorkflowWorkspace: vi.fn(),
  inspectWorkflowPurge: vi.fn(),
  purgeWorkflow: vi.fn(),
}));

vi.mock("../../services/workflowClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/workflowClient")>("../../services/workflowClient");
  return { ...actual, ...workflowMocks };
});

const catalog: RecipeViewModel[] = [];
const recipe = {
  recipeId: "R_PURGE",
  workflowVersionId: "WV_PURGE",
  version: "1.0.0",
  recipeSha256: "recipe-sha",
  inputCount: 0,
  outputCount: 1,
};
const version = {
  workflowVersionId: "WV_PURGE",
  workflowId: "WF_PURGE",
  version: "1.0.0",
  workflowSha256: "workflow-sha",
  isCurrent: true,
  enabled: false,
  archived: true,
  recipes: [recipe],
};

function workspaceResponse(): WorkflowWorkspaceQueryResponse {
  return {
    items: [{
      registry: {
        workflowId: "WF_PURGE",
        name: "Purge Workflow",
        sourceKind: "USER",
        libraryState: "REMOVED",
        currentVersionId: "WV_PURGE",
        currentVersion: version,
        currentRecipe: recipe,
        versions: [version],
        recipes: [recipe],
        projectUsageCount: 0,
        historyCount: 0,
      },
      runtime: [{
        workflowId: "WF_PURGE",
        workflowVersionId: "WV_PURGE",
        recipeId: "R_PURGE",
        name: "Purge Workflow",
        category: "video",
        mode: "text_to_video",
        workflowVersion: "1.0.0",
        recipeVersion: "1.0.0",
        workflowSha256: "workflow-sha",
        recipeSha256: "recipe-sha",
        artifactId: "ART_PURGE",
        artifactSourceKind: "USER",
        packageName: "purge-workflow-package",
        packageSourcePath: null,
        artifactStatus: "VALID",
        packageStatus: "VALID",
        libraryState: "REMOVED",
        enabled: false,
        archived: true,
        capability: "READY",
        capabilityIssues: [],
        readiness: "READY",
        readinessReasons: [],
        diagnostics: [],
        nodeCount: 1,
        hasSuccessfulRun: false,
        activeTasks: 0,
        totalTasks: 0,
      }],
    }],
    staging: [],
  };
}

function purgeInspection(canPurge: boolean, blockingReasons: string[] = []): WorkflowPurgeInspection {
  return {
    workflowId: "WF_PURGE",
    name: "Purge Workflow",
    sourceKind: "USER",
    libraryState: "REMOVED",
    taskCount: canPurge ? 0 : 1,
    batchItemCount: 0,
    presetCount: 0,
    templateCount: 0,
    shotConfigCount: 0,
    benchmarkCount: 0,
    bindingCount: 0,
    stageCount: 0,
    runTemplateCount: 0,
    packageCount: 1,
    canPurge,
    blockingReasons,
  };
}

function renderWorkspace(refreshResponse: WorkflowWorkspaceQueryResponse = { items: [], staging: [] }) {
  workflowMocks.queryWorkflowWorkspace.mockResolvedValueOnce(workspaceResponse()).mockResolvedValueOnce(refreshResponse);
  const onCatalogChanged = vi.fn().mockResolvedValue(undefined);
  render(
    <WorkflowWorkspace
      catalog={catalog}
      comfyConnected={false}
      onCatalogChanged={onCatalogChanged}
      onOpenStudio={vi.fn().mockResolvedValue(undefined)}
      onUseInProject={vi.fn().mockResolvedValue(undefined)}
    />,
  );
  return onCatalogChanged;
}

beforeEach(() => {
  vi.resetAllMocks();
  useWorkflowWorkspaceStore.getState().reset();
  useWorkflowOnboardingStore.getState().reset();
});

afterEach(() => cleanup());

describe("DEV-084 purge safety UAT", () => {
  it("后端检查阻塞时显示精确原因且不会调用永久删除", async () => {
    const user = userEvent.setup();
    workflowMocks.inspectWorkflowPurge.mockResolvedValue(purgeInspection(false, ["仍有 1 个任务引用"]));
    renderWorkspace();

    await waitFor(() => expect(workflowMocks.queryWorkflowWorkspace).toHaveBeenCalledWith("FAST"));
    await user.selectOptions(screen.getByLabelText("工作流筛选"), "archived");
    const row = within((await screen.findByText("Purge Workflow")).closest("article")!);
    await user.click(row.getByText("⋯"));
    await user.click(row.getByRole("button", { name: "彻底删除" }));

    expect((await screen.findByRole("alert")).textContent).toContain("仍有 1 个任务引用");
    expect(workflowMocks.inspectWorkflowPurge).toHaveBeenCalledWith("WF_PURGE");
    expect(workflowMocks.purgeWorkflow).not.toHaveBeenCalled();
    expect(screen.queryByRole("heading", { name: "彻底删除工作流" })).toBeNull();
  });

  it("永久删除已提交但隔离清理待补偿时显示成功而不是失败", async () => {
    const user = userEvent.setup();
    const committed: WorkflowPurgeResult = {
      workflowId: "WF_PURGE",
      versionCount: 1,
      recipeCount: 1,
      committed: true,
      cleanupPending: true,
      warning: "工作流已永久删除，但临时隔离文件清理未完成。",
    };
    workflowMocks.inspectWorkflowPurge.mockResolvedValue(purgeInspection(true));
    workflowMocks.purgeWorkflow.mockResolvedValue(committed);
    renderWorkspace();

    await waitFor(() => expect(workflowMocks.queryWorkflowWorkspace).toHaveBeenCalledWith("FAST"));
    await user.selectOptions(screen.getByLabelText("工作流筛选"), "archived");
    const row = within((await screen.findByText("Purge Workflow")).closest("article")!);
    await user.click(row.getByText("⋯"));
    await user.click(row.getByRole("button", { name: "彻底删除" }));
    const dialog = await screen.findByRole("dialog");
    const confirm = within(dialog).getByRole("button", { name: "永久删除" });
    expect((confirm as HTMLButtonElement).disabled).toBe(true);
    await user.type(within(dialog).getByLabelText('输入“永久删除”以确认'), "永久删除");
    await user.click(confirm);

    await waitFor(() => expect(workflowMocks.purgeWorkflow).toHaveBeenCalledWith("WF_PURGE"));
    const notice = await screen.findByRole("status");
    expect(notice.textContent).toContain("Purge Workflow 已永久删除，无法恢复。");
    expect(notice.textContent).toContain("部分隔离临时文件尚未清理，不影响删除结果。");
    expect(notice.textContent).not.toContain("删除失败");
  });
});
