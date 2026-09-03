// @vitest-environment jsdom

import type { ReactNode } from "react";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../types/generation";
import type {
  ProjectWorkflowBindingView,
  ProjectWorkflowConfigView,
} from "../types/projectWorkflow";
import { EMPTY_WORKSPACE_RESUME, useWorkspaceResumeStore } from "../stores/workspaceResumeStore";
import { useProjectStore } from "../stores/projectStore";
import App from "./App";

const mocks = vi.hoisted(() => ({
  listGenerationCatalog: vi.fn(),
  getProjectWorkflowConfig: vi.fn(),
  replaceProjectWorkflowConfig: vi.fn(),
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
vi.mock("./StudioShell", () => ({
  StudioShell: ({ children, onNavigate }: { children: ReactNode; onNavigate: (destination: unknown, item: { id: string }) => void }) => (
    <>
      <button type="button" onClick={() => onNavigate("workflows", { id: "workflows" })}>工作流</button>
      {children}
    </>
  ),
}));
vi.mock("./StartupScreen", () => ({ StartupScreen: ({ onRetry }: { onRetry: () => void }) => <button onClick={onRetry}>重试</button> }));
vi.mock("./WorkspaceErrorBoundary", () => ({ WorkspaceErrorBoundary: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("../features/workflows/WorkflowWorkspace", () => ({
  WorkflowWorkspace: ({ onUseInProject }: { onUseInProject: (workflowId: string, recipeId: string) => Promise<void> }) => (
    <button
      type="button"
      onClick={() => void onUseInProject(mocks.selectedWorkflow.workflowId, mocks.selectedWorkflow.recipeId)}
    >
      用于当前项目
    </button>
  ),
}));
vi.mock("../features/projects/ProjectCommandCenter", () => ({ ProjectCommandCenter: () => null }));
vi.mock("../features/projects/ProjectWorkspace", () => ({ ProjectWorkspace: () => null }));
vi.mock("../features/studio/GenerationStudio", () => ({ GenerationStudio: () => null }));
vi.mock("../features/assets/AssetWorkspace", () => ({ AssetWorkspace: () => null }));
vi.mock("../features/assets/AssetVideoBatchWorkspace", () => ({ AssetVideoBatchWorkspace: () => null }));
vi.mock("../features/tasks/TaskHistory", () => ({ TaskHistory: () => null }));
vi.mock("../features/shots/ShotWorkspace", () => ({ ShotWorkspace: () => null }));
vi.mock("../features/settings/SettingsWorkspace", () => ({ SettingsWorkspace: () => null }));

const project = {
  id: "project-1",
  name: "UAT Project",
  description: "",
  createdAt: "2026-09-03T00:00:00.000Z",
  updatedAt: "2026-09-03T00:00:00.000Z",
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

function binding(
  stage: "IMAGE" | "VIDEO",
  mode: ProjectWorkflowBindingView["mode"],
  workflowVersionId: string,
  recipeId: string,
): ProjectWorkflowBindingView {
  return {
    stage,
    mode,
    workflowVersionId,
    recipeId,
    createdAt: "2026-09-03T00:00:00.000Z",
    updatedAt: "2026-09-03T00:00:00.000Z",
    available: true,
  };
}

function config(
  imageDefault: ProjectWorkflowBindingView | null | undefined,
  videoDefault: ProjectWorkflowBindingView | null | undefined,
  videoModeOverrides: ProjectWorkflowBindingView[] = [],
): ProjectWorkflowConfigView {
  return { projectId: project.id, imageDefault, videoDefault, videoModeOverrides };
}

async function openWorkflowAction() {
  const user = userEvent.setup();
  await waitFor(() => expect(screen.getByRole("button", { name: "工作流" })).toBeTruthy());
  await user.click(screen.getByRole("button", { name: "工作流" }));
  await user.click(await screen.findByRole("button", { name: "用于当前项目" }));
}

function prepareApp(catalog: RecipeViewModel[], hasProject = true) {
  mocks.listGenerationCatalog.mockResolvedValue(catalog);
  mocks.listProjects.mockResolvedValue(hasProject ? [project] : []);
  mocks.getWorkspaceResume.mockResolvedValue(EMPTY_WORKSPACE_RESUME);
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

describe("DEV-080 用于当前项目持久化 UAT", () => {
  it("保存 image 默认，并保留 video 默认和全部 video mode overrides", async () => {
    const selected = recipe(["image"]);
    const videoDefault = binding("VIDEO", "DEFAULT", "OLD_VIDEO_WV", "OLD_VIDEO_R");
    const overrides = [
      binding("VIDEO", "FL2VA_TEXT_TO_VIDEO", "OVERRIDE_WV_1", "OVERRIDE_R_1"),
      binding("VIDEO", "FL2VA_IMAGE_TO_VIDEO", "OVERRIDE_WV_2", "OVERRIDE_R_2"),
      binding("VIDEO", "FL2VA_FIRST_LAST", "OVERRIDE_WV_3", "OVERRIDE_R_3"),
      binding("VIDEO", "REF2VA_IMAGE", "OVERRIDE_WV_4", "OVERRIDE_R_4"),
      binding("VIDEO", "REF2VA_AUDIO", "OVERRIDE_WV_5", "OVERRIDE_R_5"),
      binding("VIDEO", "REF2VA_IMAGE_AUDIO", "OVERRIDE_WV_6", "OVERRIDE_R_6"),
      binding("VIDEO", "REF2VA_VIDEO_IMAGE", "OVERRIDE_WV_7", "OVERRIDE_R_7"),
    ];
    const current = config(binding("IMAGE", "DEFAULT", "OLD_IMAGE_WV", "OLD_IMAGE_R"), videoDefault, overrides);
    const saved = config(binding("IMAGE", "DEFAULT", selected.workflowVersionId, selected.recipeId), videoDefault, overrides);
    mocks.getProjectWorkflowConfig.mockResolvedValue(current);
    mocks.replaceProjectWorkflowConfig.mockResolvedValue(saved);
    prepareApp([selected]);

    await openWorkflowAction();

    await waitFor(() => expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledTimes(1));
    expect(mocks.listGenerationCatalog.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledWith("project-1", {
      bindings: [
        { stage: "IMAGE", mode: "DEFAULT", workflowVersionId: selected.workflowVersionId, recipeId: selected.recipeId },
        { stage: "VIDEO", mode: "DEFAULT", workflowVersionId: "OLD_VIDEO_WV", recipeId: "OLD_VIDEO_R" },
        ...overrides.map(({ stage, mode, workflowVersionId, recipeId }) => ({ stage, mode, workflowVersionId, recipeId })),
      ],
    });
    await waitFor(() => expect(document.querySelector(".workflow-notice")?.textContent).toContain("已设为当前项目图片默认工作流"));
  });

  it("保存 video 默认，并保留 image 默认和全部 video mode overrides", async () => {
    mocks.selectedWorkflow.workflowId = "VIDEO_WF";
    mocks.selectedWorkflow.recipeId = "VIDEO_R";
    const selected = recipe(["video"]);
    const imageDefault = binding("IMAGE", "DEFAULT", "OLD_IMAGE_WV", "OLD_IMAGE_R");
    const overrides = [binding("VIDEO", "REF2VA_IMAGE", "OVERRIDE_WV", "OVERRIDE_R")];
    const current = config(imageDefault, binding("VIDEO", "DEFAULT", "OLD_VIDEO_WV", "OLD_VIDEO_R"), overrides);
    const saved = config(imageDefault, binding("VIDEO", "DEFAULT", selected.workflowVersionId, selected.recipeId), overrides);
    mocks.getProjectWorkflowConfig.mockResolvedValue(current);
    mocks.replaceProjectWorkflowConfig.mockResolvedValue(saved);
    prepareApp([selected]);

    await openWorkflowAction();

    await waitFor(() => expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledTimes(1));
    expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledWith("project-1", {
      bindings: [
        { stage: "IMAGE", mode: "DEFAULT", workflowVersionId: "OLD_IMAGE_WV", recipeId: "OLD_IMAGE_R" },
        { stage: "VIDEO", mode: "DEFAULT", workflowVersionId: selected.workflowVersionId, recipeId: selected.recipeId },
        ...overrides.map(({ stage, mode, workflowVersionId, recipeId }) => ({ stage, mode, workflowVersionId, recipeId })),
      ],
    });
    await waitFor(() => expect(document.querySelector(".workflow-notice")?.textContent).toContain("已设为当前项目视频默认工作流"));
  });

  it("没有项目时拒绝绑定", async () => {
    const selected = recipe(["image"]);
    prepareApp([selected], false);

    await openWorkflowAction();

    await waitFor(() => expect(document.querySelector(".global-error")?.textContent).toContain("当前项目不可用，无法绑定工作流。"));
    expect(mocks.getProjectWorkflowConfig).not.toHaveBeenCalled();
    expect(mocks.replaceProjectWorkflowConfig).not.toHaveBeenCalled();
  });

  it("replace 返回非 exact pair 时拒绝确认", async () => {
    const selected = recipe(["image"]);
    mocks.getProjectWorkflowConfig.mockResolvedValue(config(null, null));
    mocks.replaceProjectWorkflowConfig.mockResolvedValue(
      config(binding("IMAGE", "DEFAULT", "WRONG_WV", "WRONG_R"), null),
    );
    prepareApp([selected]);

    await openWorkflowAction();

    await waitFor(() => expect(document.querySelector(".global-error")?.textContent).toContain("绑定写入后校验失败"));
    expect(document.querySelector(".workflow-notice")).toBeNull();
  });

  it("目录没有 exact workflowId + recipeId 时拒绝绑定", async () => {
    mocks.selectedWorkflow.workflowId = "MISSING_WF";
    mocks.selectedWorkflow.recipeId = "MISSING_R";
    const requested = recipe(["image"]);
    prepareApp([{ ...requested, recipeId: "OTHER_R" }]);

    await openWorkflowAction();

    await waitFor(() => expect(document.querySelector(".global-error")?.textContent).toContain("刚添加的工作流暂时还没有出现在项目工作流列表中，请刷新后重试。"));
    expect(mocks.getProjectWorkflowConfig).not.toHaveBeenCalled();
    expect(mocks.replaceProjectWorkflowConfig).not.toHaveBeenCalled();
  });

  it("没有输出时拒绝绑定", async () => {
    const selected = recipe();
    prepareApp([selected]);

    await openWorkflowAction();

    await waitFor(() => expect(document.querySelector(".global-error")?.textContent).toContain("该工作流没有图片或视频输出，无法绑定为项目默认工作流。"));
    expect(mocks.getProjectWorkflowConfig).not.toHaveBeenCalled();
    expect(mocks.replaceProjectWorkflowConfig).not.toHaveBeenCalled();
  });

  it("双输出时进入项目设置但不猜测默认阶段", async () => {
    const selected = recipe(["image", "video"]);
    const current = config(null, null);
    mocks.getProjectWorkflowConfig.mockResolvedValue(current);
    prepareApp([selected]);

    await openWorkflowAction();

    await waitFor(() => expect(document.querySelector(".workflow-notice")?.textContent).toContain("同时输出图片和视频，未自动绑定"));
    expect(mocks.getProjectWorkflowConfig).not.toHaveBeenCalled();
    expect(mocks.replaceProjectWorkflowConfig).not.toHaveBeenCalled();
  });
});
