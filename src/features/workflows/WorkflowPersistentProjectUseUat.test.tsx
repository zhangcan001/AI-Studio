// @vitest-environment jsdom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import { useWorkflowOnboardingStore } from "../../stores/workflowOnboardingStore";
import { useWorkflowWorkspaceStore } from "../../stores/workflowWorkspaceStore";
import type {
  WorkflowDeletionInspection,
  WorkflowProductionWorkspaceResponse,
  WorkflowProductionWorkspaceView,
  WorkflowRestoreResult,
} from "../../types/workflowOnboarding";
import { WorkflowWorkspace } from "./WorkflowWorkspace";

const tauriMocks = vi.hoisted(() => ({
  listWorkflowProductionWorkspace: vi.fn(),
  refreshWorkflowProductionWorkspace: vi.fn(),
  inspectWorkflowDeletion: vi.fn(),
  deleteWorkflowVersion: vi.fn(),
  restoreWorkflowVersion: vi.fn(),
  recheckWorkflowCapability: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, ...tauriMocks };
});

const catalogRecipe: RecipeViewModel = {
  workflowId: "WF1",
  workflowVersionId: "WV1",
  recipeId: "R2",
  name: "Published Workflow",
  category: "image",
  mode: "text_to_image",
  fields: [],
  outputTypes: ["image"],
};

const publishedRow: WorkflowProductionWorkspaceView = {
  packageName: "published-workflow-package",
  builtin: false,
  archived: false,
  packageStatus: "VALID",
  workflowId: "WF1",
  workflowVersionId: "WV1",
  name: "Published Workflow",
  workflowVersion: "1.0.0",
  enabled: true,
  capability: "READY",
  readiness: "READY",
  readinessReasons: [],
  capabilityIssues: [],
  nodeCount: 1,
  recipes: [
    { recipeId: "R1", version: "1.0.0", inputCount: 0, outputCount: 1 },
    { recipeId: "R2", version: "2.0.0", inputCount: 0, outputCount: 1 },
  ],
  activeTasks: 0,
  totalTasks: 0,
  hasSuccessfulRun: false,
  diagnostics: [],
};

const archivedRow: WorkflowProductionWorkspaceView = {
  ...publishedRow,
  packageName: "archived-workflow-package",
  archived: true,
  workflowId: "ARCHIVED_WF",
  workflowVersionId: "ARCHIVED_WV",
  name: "Archived Workflow",
  enabled: false,
  recipes: [{ recipeId: "ARCHIVED_R1", version: "1.0.0", inputCount: 0, outputCount: 1 }],
};

const readyRestoreResult: WorkflowRestoreResult = {
  workflowVersionId: "ARCHIVED_WV",
  archived: false,
  enabled: true,
  capability: "READY",
  readiness: "READY",
};

const productDeletionInspection: WorkflowDeletionInspection = {
  workflowId: "PRODUCT_WF",
  workflowVersionId: "PRODUCT_WV",
  name: "Product Workflow",
  builtin: true,
  enabled: true,
  archived: false,
  activeTaskCount: 0,
  activeQueueItemCount: 0,
  historicalTaskCount: 2,
  productionBatchItemCount: 0,
  benchmarkReferenceCount: 0,
  projectBindingCount: 2,
  canHardDelete: false,
  requiresArchive: true,
  blockingReasons: [],
  deleteAction: "REMOVE",
};

function workspaceResponse(items: WorkflowProductionWorkspaceView[]): WorkflowProductionWorkspaceResponse {
  return { items, staging: [] };
}

function renderWorkspace({
  projectId,
  catalog = [catalogRecipe],
  items = [publishedRow],
  refreshItems = items,
  restoreResult = readyRestoreResult,
}: {
  projectId?: string;
  catalog?: RecipeViewModel[];
  items?: WorkflowProductionWorkspaceView[];
  refreshItems?: WorkflowProductionWorkspaceView[];
  restoreResult?: WorkflowRestoreResult;
} = {}) {
  tauriMocks.listWorkflowProductionWorkspace.mockResolvedValue(workspaceResponse(items));
  tauriMocks.refreshWorkflowProductionWorkspace.mockResolvedValue(workspaceResponse(refreshItems));
  tauriMocks.restoreWorkflowVersion.mockResolvedValue(restoreResult);
  tauriMocks.recheckWorkflowCapability.mockResolvedValue({ state: "READY", issues: [] });
  const onUseInProject = vi.fn().mockResolvedValue(undefined);

  render(
    <WorkflowWorkspace
      projectId={projectId}
      catalog={catalog}
      comfyConnected={false}
      onCatalogChanged={vi.fn().mockResolvedValue(undefined)}
      onOpenStudio={vi.fn().mockResolvedValue(undefined)}
      onUseInProject={onUseInProject}
    />,
  );

  return onUseInProject;
}

async function publishedRowView(name = "Published Workflow") {
  await waitFor(() => expect(tauriMocks.listWorkflowProductionWorkspace).toHaveBeenCalledTimes(1));
  const rowName = await screen.findByText(name);
  return within(rowName.closest("article")!);
}

beforeEach(() => {
  vi.clearAllMocks();
  useWorkflowWorkspaceStore.getState().reset();
  useWorkflowOnboardingStore.getState().reset();
});

afterEach(() => cleanup());

describe("DEV-079 工作流列表用于当前项目 UAT", () => {
  it("有项目且目录存在精确 Recipe 时启用按钮，并传递 workflowId 与 R2", async () => {
    const user = userEvent.setup();
    const onUseInProject = renderWorkspace({ projectId: "project-1" });

    const row = await publishedRowView();
    const button = row.getByRole("button", { name: "用于当前项目" });
    expect((button as HTMLButtonElement).disabled).toBe(false);

    await user.click(button);

    expect(onUseInProject).toHaveBeenCalledTimes(1);
    expect(onUseInProject).toHaveBeenCalledWith("WF1", "R2");
  });

  it("没有项目时仍显示按钮但禁用，并提示先选择或创建项目", async () => {
    renderWorkspace();

    const button = (await publishedRowView()).getByRole("button", { name: "用于当前项目" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    expect(button.getAttribute("title")).toBe("请先选择或创建一个项目");
  });

  it("有项目但没有精确目录 Recipe 时仍显示按钮但禁用，并提示尚未进入生产目录", async () => {
    renderWorkspace({ projectId: "project-1", catalog: [] });

    const button = (await publishedRowView()).getByRole("button", { name: "用于当前项目" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    expect(button.getAttribute("title")).toBe("该工作流尚未进入生产目录");
  });

  it("归档行不暴露用于当前项目按钮", async () => {
    const user = userEvent.setup();
    renderWorkspace({ projectId: "project-1", items: [publishedRow, archivedRow] });

    await waitFor(() => expect(tauriMocks.listWorkflowProductionWorkspace).toHaveBeenCalledTimes(1));
    await user.selectOptions(screen.getByLabelText("工作流筛选"), "archived");

    const archivedName = await screen.findByText("Archived Workflow");
    const archivedView = within(archivedName.closest("article")!);
    expect(archivedView.queryByRole("button", { name: "用于当前项目" })).toBeNull();
    expect(archivedView.getAllByText(/已删除/).length).toBeGreaterThan(0);
    await user.click(archivedView.getByRole("button", { name: "恢复工作流" }));
    await waitFor(() => expect(tauriMocks.restoreWorkflowVersion).toHaveBeenCalledWith("ARCHIVED_WV"));
    expect(tauriMocks.recheckWorkflowCapability).not.toHaveBeenCalled();
    expect((await screen.findByRole("status")).textContent).toContain("Archived Workflow 已恢复并重新启用，现在可以正常使用。");
  });

  it("缺少节点时恢复成功但保持停用", async () => {
    const user = userEvent.setup();
    const blockedResult: WorkflowRestoreResult = {
      ...readyRestoreResult,
      enabled: false,
      capability: "MISSING_NODES",
      readiness: "BLOCKED",
    };
    renderWorkspace({
      items: [archivedRow],
      restoreResult: blockedResult,
      refreshItems: [{ ...archivedRow, archived: false, enabled: false, capability: "MISSING_NODES", readiness: "BLOCKED" }],
    });

    await waitFor(() => expect(tauriMocks.listWorkflowProductionWorkspace).toHaveBeenCalledTimes(1));
    await user.selectOptions(screen.getByLabelText("工作流筛选"), "archived");
    const archivedView = within((await screen.findByText("Archived Workflow")).closest("article")!);
    await user.click(archivedView.getByRole("button", { name: "恢复工作流" }));

    await waitFor(() => expect(tauriMocks.restoreWorkflowVersion).toHaveBeenCalledWith("ARCHIVED_WV"));
    expect(tauriMocks.recheckWorkflowCapability).not.toHaveBeenCalled();
    expect((await screen.findByRole("status")).textContent).toContain("Archived Workflow 已恢复，但当前缺少 ComfyUI 节点，暂时保持停用。");
  });

  it("ComfyUI 离线时恢复成功但保持停用", async () => {
    const user = userEvent.setup();
    const offlineResult: WorkflowRestoreResult = {
      ...readyRestoreResult,
      enabled: false,
      capability: "COMFY_OFFLINE",
      readiness: "BLOCKED",
    };
    renderWorkspace({
      items: [archivedRow],
      restoreResult: offlineResult,
      refreshItems: [{ ...archivedRow, archived: false, enabled: false, capability: "COMFY_OFFLINE", readiness: "BLOCKED" }],
    });

    await waitFor(() => expect(tauriMocks.listWorkflowProductionWorkspace).toHaveBeenCalledTimes(1));
    await user.selectOptions(screen.getByLabelText("工作流筛选"), "archived");
    const archivedView = within((await screen.findByText("Archived Workflow")).closest("article")!);
    await user.click(archivedView.getByRole("button", { name: "恢复工作流" }));

    await waitFor(() => expect(tauriMocks.restoreWorkflowVersion).toHaveBeenCalledWith("ARCHIVED_WV"));
    expect(tauriMocks.recheckWorkflowCapability).not.toHaveBeenCalled();
    expect((await screen.findByRole("status")).textContent).toContain("Archived Workflow 已恢复，但当前 ComfyUI 离线，暂时保持停用。");
  });

  it("PRODUCT 删除成功提示使用返回的 binding 数量并只在有历史时保留历史提示", async () => {
    const user = userEvent.setup();
    const productRow: WorkflowProductionWorkspaceView = {
      ...publishedRow,
      packageName: "product-workflow-package",
      workflowId: "PRODUCT_WF",
      workflowVersionId: "PRODUCT_WV",
      name: "Product Workflow",
      builtin: true,
    };
    tauriMocks.inspectWorkflowDeletion.mockResolvedValue({ ...productDeletionInspection, projectBindingCount: 1 });
    tauriMocks.deleteWorkflowVersion.mockResolvedValue({
      action: "REMOVE",
      projectBindingCount: 2,
      workflowId: "PRODUCT_WF",
      workflowVersionId: "PRODUCT_WV",
      archived: true,
    });
    renderWorkspace({ items: [productRow] });

    const row = await publishedRowView("Product Workflow");
    await user.click(row.getByRole("button", { name: "删除" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "删除工作流" }));

    const notice = await screen.findByRole("status");
    expect(notice.textContent).toContain("已解除 2 项项目工作流配置");
    expect(notice.textContent).toContain("历史生产记录仍然保留");
    expect(notice.textContent).toContain("可在“已删除”中恢复");
  });
});
