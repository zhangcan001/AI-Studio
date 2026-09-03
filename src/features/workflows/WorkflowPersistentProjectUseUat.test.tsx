// @vitest-environment jsdom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import { useWorkflowOnboardingStore } from "../../stores/workflowOnboardingStore";
import { useWorkflowWorkspaceStore } from "../../stores/workflowWorkspaceStore";
import type {
  WorkflowProductionWorkspaceResponse,
  WorkflowProductionWorkspaceView,
} from "../../types/workflowOnboarding";
import { WorkflowWorkspace } from "./WorkflowWorkspace";

const tauriMocks = vi.hoisted(() => ({
  listWorkflowProductionWorkspace: vi.fn(),
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

function workspaceResponse(items: WorkflowProductionWorkspaceView[]): WorkflowProductionWorkspaceResponse {
  return { items, staging: [] };
}

function renderWorkspace({
  projectId,
  catalog = [catalogRecipe],
  items = [publishedRow],
}: {
  projectId?: string;
  catalog?: RecipeViewModel[];
  items?: WorkflowProductionWorkspaceView[];
} = {}) {
  tauriMocks.listWorkflowProductionWorkspace.mockResolvedValue(workspaceResponse(items));
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

async function publishedRowView() {
  await waitFor(() => expect(tauriMocks.listWorkflowProductionWorkspace).toHaveBeenCalledTimes(1));
  const name = await screen.findByText("Published Workflow");
  return within(name.closest("article")!);
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
  });
});
