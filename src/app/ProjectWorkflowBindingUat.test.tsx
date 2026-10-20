// @vitest-environment jsdom

import type { ReactNode } from "react";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../types/generation";
import { EMPTY_WORKSPACE_RESUME, useWorkspaceResumeStore } from "../stores/workspaceResumeStore";
import { useProjectStore } from "../stores/projectStore";
import App from "./App";

const mocks = vi.hoisted(() => ({
  listGenerationCatalog: vi.fn(),
  getProjectWorkflowConfig: vi.fn(),
  upsertProjectWorkflowBinding: vi.fn(),
  removeProjectWorkflowBinding: vi.fn(),
  listProjects: vi.fn(),
  getWorkspaceResume: vi.fn(),
  saveWorkspaceResume: vi.fn(),
  getProductionAdmissionStatus: vi.fn(),
  getRuntimeActivityStatus: vi.fn(),
  listRecentTasks: vi.fn(),
  listConsistencyProfiles: vi.fn(),
  listReferenceSets: vi.fn(),
  listCostumeVariants: vi.fn(),
  listShots: vi.fn(),
  selectedWorkflow: { workflowId: "IMAGE_WF", recipeId: "IMAGE_R" },
}));

const bootstrapMock = vi.hoisted(() => ({ bootstrap: vi.fn() }));
const taskEventsMock = vi.hoisted(() => ({ subscribeTaskUpdates: vi.fn() }));

vi.mock("../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../services/tauriClient")>("../services/tauriClient");
  return { ...actual, ...mocks };
});

vi.mock("./bootstrap", () => bootstrapMock);
vi.mock("../services/taskEvents", () => taskEventsMock);
vi.mock("./ShellHost", () => ({
  ShellHost: ({ children, navigate }: { children: ReactNode; navigate: (route: import("./routes/types").AppRoute) => void }) => (
    <>
      <button type="button" onClick={() => navigate({ kind: "system-settings", section: "advanced-workflows" })}>工作流</button>
      {children}
    </>
  ),
}));
vi.mock("./StartupScreen", () => ({ StartupScreen: ({ onRetry }: { onRetry: () => void }) => <button onClick={onRetry}>重试</button> }));
vi.mock("./WorkspaceErrorBoundary", () => ({ WorkspaceErrorBoundary: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("./v3/ProjectOverviewPage", () => ({
  ProjectOverviewPage: ({ navigate }: { navigate: (route: import("./routes/types").AppRoute) => void }) => (
    <button
      type="button"
      onClick={() => navigate({ kind: "create", projectId: "project-2", shotId: "shot-2", stage: "image" })}
    >
      打开跨项目镜头
    </button>
  ),
}));
vi.mock("../features/projects/ProjectWorkspace", () => ({ ProjectWorkspace: () => null }));
vi.mock("../features/assets/AssetWorkspace", () => ({ AssetWorkspace: () => null }));
vi.mock("../features/assets/AssetVideoBatchWorkspace", () => ({ AssetVideoBatchWorkspace: () => null }));
vi.mock("../features/tasks/TaskHistory", () => ({ TaskHistory: () => null }));
vi.mock("../features/create/CreatePage", () => ({
  CreatePage: ({ route }: { route: Extract<import("./routes/types").AppRoute, { kind: "create" }> }) => (
    <output data-testid="shot-workspace-target">{route.projectId}:{route.shotId ?? "none"}:creation</output>
  ),
}));
vi.mock("../features/settings/SettingsWorkspace", () => ({ SettingsWorkspace: () => null }));

const project = {
  id: "project-1",
  name: "UAT Project",
  description: "",
  createdAt: "2026-09-03T00:00:00.000Z",
  updatedAt: "2026-09-03T00:00:00.000Z",
};

const secondProject = {
  ...project,
  id: "project-2",
  name: "Second Project",
};

function recipe(outputTypes?: Array<"image" | "video">): RecipeViewModel {
  return {
    workflowId: mocks.selectedWorkflow.workflowId,
    workflowVersionId: `${mocks.selectedWorkflow.workflowId}_VERSION`,
    recipeId: mocks.selectedWorkflow.recipeId,
    name: `${mocks.selectedWorkflow.workflowId} recipe`,
    category: outputTypes?.includes("video") ? "video" : "image",
    mode: "custom",
    fields: [],
    outputTypes,
  };
}

async function openWorkflowAction() {
  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: "工作流" }));
  await screen.findByRole("heading", { name: "工作流导入与编辑已停用" });
}

function prepareApp(catalog: RecipeViewModel[], hasProject = true, projectList = hasProject ? [project] : []) {
  mocks.listGenerationCatalog.mockResolvedValue(catalog);
  mocks.listProjects.mockResolvedValue(projectList);
  // This fixture opens a chosen project. A genuinely empty resume now starts
  // at the canonical project list, rather than silently selecting a project.
  mocks.getWorkspaceResume.mockResolvedValue(hasProject ? { ...EMPTY_WORKSPACE_RESUME, lastProjectId: projectList[0]?.id, lastWorkspace: "command-center" } : EMPTY_WORKSPACE_RESUME);
  mocks.saveWorkspaceResume.mockImplementation(async (resume: unknown) => resume);
  mocks.getProductionAdmissionStatus.mockResolvedValue({ busy: false });
  mocks.getRuntimeActivityStatus.mockResolvedValue({ activeTaskCount: 0, productionBusy: false });
  mocks.listRecentTasks.mockResolvedValue([]);
  mocks.listConsistencyProfiles.mockResolvedValue([]);
  mocks.listReferenceSets.mockResolvedValue([]);
  mocks.listCostumeVariants.mockResolvedValue([]);
  mocks.listShots.mockResolvedValue([]);
  bootstrapMock.bootstrap.mockResolvedValue({
    ping: "pong",
    status: { backend: "ready", database: "ready", version: "1.0.0" },
    comfy: { status: "OFFLINE", endpoint: "http://127.0.0.1:8188", devices: [] },
  });
  taskEventsMock.subscribeTaskUpdates.mockResolvedValue(vi.fn());

  render(<App />);
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  // Deprecated preference must no longer select a legacy UI; binding invariants remain.
  localStorage.setItem("aistudio.shellMode", "legacy");
  useProjectStore.setState({ projects: [], activeProjectId: undefined, loading: true, error: undefined });
  useWorkspaceResumeStore.setState({
    resume: EMPTY_WORKSPACE_RESUME,
    loaded: false,
    saving: false,
    error: undefined,
  });
  mocks.selectedWorkflow.workflowId = "IMAGE_WF";
  mocks.selectedWorkflow.recipeId = "IMAGE_R";
});

afterEach(() => cleanup());

describe("Retired workflow route real-page UAT", () => {
  it.each([["image"], ["video"], ["image", "video"], []] as Array<Array<"image" | "video">>)("never binds a generator for catalog outputs %j", async (...outputs) => {
    prepareApp([recipe(outputs)]);
    await openWorkflowAction();
    expect(screen.queryByRole("button", { name: "用于当前项目" })).toBeNull();
    expect(mocks.getProjectWorkflowConfig).not.toHaveBeenCalled();
    expect(mocks.upsertProjectWorkflowBinding).not.toHaveBeenCalled();
    expect(mocks.removeProjectWorkflowBinding).not.toHaveBeenCalled();
    expect(mocks.listGenerationCatalog).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "选择项目生成器" })).toBeTruthy();
  });
  it("an absent active project has navigation only and cannot mutate bindings", async () => {
    prepareApp([], false);
    await openWorkflowAction();
    expect(screen.getByRole("button", { name: "选择项目" })).toBeTruthy();
    expect(mocks.getProjectWorkflowConfig).not.toHaveBeenCalled();
    expect(mocks.upsertProjectWorkflowBinding).not.toHaveBeenCalled();
  });

  it("切换到目标项目后再应用精确镜头焦点", async () => {
    prepareApp([recipe(["image"])], true, [project, secondProject]);

    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "打开跨项目镜头" }));

    await waitFor(() => expect(screen.getByTestId("shot-workspace-target").textContent).toBe("project-2:shot-2:creation"));
    expect(useProjectStore.getState().activeProjectId).toBe("project-2");
  });
});
