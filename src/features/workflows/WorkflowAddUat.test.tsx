// @vitest-environment jsdom

import { useState } from "react";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type { ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import type {
  WorkflowAutoOnboardingPlanView,
  WorkflowOnboardingDraftView,
  WorkflowProductionWorkspaceResponse,
} from "../../types/workflowOnboarding";
import { useWorkflowOnboardingStore } from "../../stores/workflowOnboardingStore";
import { useWorkflowWorkspaceStore } from "../../stores/workflowWorkspaceStore";
import { ProjectWorkflowSettings } from "../projects/ProjectWorkflowSettings";
import { WorkflowImportFormatIssue } from "./WorkflowImportIssues";
import { WorkflowSmartImport, workflowImportFormat } from "./WorkflowSmartImport";
import { WorkflowWorkspace } from "./WorkflowWorkspace";

const serviceMocks = vi.hoisted(() => ({
  autoOnboardWorkflow: vi.fn(),
  getOnboardingDraft: vi.fn(),
  setOnboardingInputMapping: vi.fn(),
  listWorkflowProductionWorkspace: vi.fn(),
  refreshWorkflowProductionWorkspace: vi.fn(),
  listGenerationCatalog: vi.fn(),
  getProjectWorkflowConfig: vi.fn(),
  replaceProjectWorkflowConfig: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, ...serviceMocks };
});

beforeEach(() => {
  vi.clearAllMocks();
  useWorkflowOnboardingStore.getState().reset();
  useWorkflowWorkspaceStore.getState().reset();
});

afterEach(() => cleanup());

const EMPTY_WORKSPACE = {
  items: [],
  staging: [],
} satisfies WorkflowProductionWorkspaceResponse;

const EMPTY_PROJECT_CONFIG: ProjectWorkflowConfigView = {
  projectId: "project-1",
  videoModeOverrides: [],
};

function plan(overrides: Partial<WorkflowAutoOnboardingPlanView> = {}): WorkflowAutoOnboardingPlanView {
  return {
    draftId: "draft-1",
    state: "AUTO_PUBLISHED",
    workflowKind: "VIDEO",
    workflowSha256: "sha-1",
    originalFilename: "demo.json",
    nodeCount: 4,
    uniqueClassCount: 3,
    metadata: {
      workflowId: "workflow-1",
      name: "Demo Workflow",
      workflowVersion: "1.0.0",
      recipeVersion: "1.0.0",
      category: "video",
      mode: "CUSTOM_VIDEO",
      recipeId: "recipe-1",
    },
    capability: { state: "READY", issues: [] },
    inputMappings: [{
      semanticKey: "prompt",
      fieldType: "textarea",
      label: "提示词",
      required: true,
      targetNode: "1",
      targetInput: "text",
    }],
    outputMappings: [{ outputId: "output_1", label: "视频", type: "video", nodeId: "4", required: true }],
    validation: {
      apiFormat: true,
      recipe: true,
      bindings: true,
      outputs: true,
      manifest: true,
      capability: true,
      dryRun: true,
      readyToPublish: true,
      issues: [],
    },
    inferences: [],
    issues: [],
    autoPublishable: true,
    published: {
      workflowId: "workflow-1",
      workflowVersion: "1.0.0",
      recipeId: "recipe-1",
      packageName: "Demo Workflow",
      workflowSha256: "sha-1",
      refreshed: { packagesFound: 1, valid: 1, invalid: 0, inserted: 1, reused: 0, errors: [] },
    },
    message: "published",
    ...overrides,
  };
}

function smartImportProps() {
  return {
    loading: false,
    onResolve: vi.fn(),
    onResume: vi.fn(),
    onOpenAdvanced: vi.fn(),
    onOpenExisting: vi.fn(),
  };
}

function addedRecipe(kind: "image" | "video"): RecipeViewModel {
  const label = kind === "image" ? "图片" : "视频";
  return {
    workflowId: `dev079-${kind}-workflow`,
    workflowVersionId: `dev079-${kind}-workflow-version-2`,
    recipeId: `dev079-${kind}-recipe-2`,
    name: `DEV-079 ${label} 工作流`,
    category: kind,
    mode: kind === "image" ? "text_to_image" : "custom_video",
    fields: [{ key: "prompt", type: "textarea", label: "提示词", required: true, default: "" }],
    outputTypes: [kind],
  };
}

function publishedPlan(recipe: RecipeViewModel): WorkflowAutoOnboardingPlanView {
  const outputType = recipe.outputTypes?.includes("video") ? "video" : "image";
  const metadata = {
    workflowId: recipe.workflowId,
    name: recipe.name,
    workflowVersion: "2.0.0",
    recipeVersion: "1.0.0",
    category: recipe.category,
    mode: recipe.mode,
    recipeId: recipe.recipeId,
  };
  return {
    draftId: "dev079-p1-draft",
    state: "AUTO_PUBLISHED",
    workflowKind: outputType === "video" ? "VIDEO" : "IMAGE",
    workflowSha256: "dev079-p1-sha",
    originalFilename: `${recipe.name}.json`,
    nodeCount: 1,
    uniqueClassCount: 1,
    metadata,
    capability: { state: "READY", issues: [] },
    inputMappings: [],
    outputMappings: [{ outputId: "output_1", label: outputType === "video" ? "视频" : "图片", type: outputType, nodeId: "1", required: true }],
    validation: {
      apiFormat: true,
      recipe: true,
      bindings: true,
      outputs: true,
      manifest: true,
      capability: true,
      dryRun: true,
      readyToPublish: true,
      issues: [],
    },
    inferences: [],
    issues: [],
    autoPublishable: true,
    published: {
      workflowId: recipe.workflowId,
      workflowVersion: metadata.workflowVersion,
      recipeId: recipe.recipeId,
      packageName: recipe.name,
      workflowSha256: "dev079-p1-sha",
      refreshed: { packagesFound: 1, valid: 1, invalid: 0, inserted: 1, reused: 0, errors: [] },
    },
    message: "工作流已添加到列表。",
  };
}

function onboardingDraft(recipe: RecipeViewModel): WorkflowOnboardingDraftView {
  const plan = publishedPlan(recipe);
  return {
    draftId: plan.draftId,
    workflowSha256: plan.workflowSha256,
    originalFilename: plan.originalFilename,
    nodeCount: 1,
    uniqueClassCount: 1,
    nodes: [{ nodeId: "1", classType: "OutputNode", title: "输出", isOutputNode: true, inputs: [] }],
    capability: { state: "READY", issues: [] },
    inputMappings: [],
    outputMappings: plan.outputMappings,
    manifest: plan.metadata,
    recipe: { inputs: [], bindings: [], outputs: plan.outputMappings, valid: true, issues: [] },
    validation: plan.validation,
  };
}

function prepareTauriBoundary(recipe: RecipeViewModel) {
  serviceMocks.autoOnboardWorkflow.mockResolvedValue(publishedPlan(recipe));
  serviceMocks.getOnboardingDraft.mockResolvedValue(onboardingDraft(recipe));
  serviceMocks.listWorkflowProductionWorkspace.mockResolvedValue(EMPTY_WORKSPACE);
  serviceMocks.refreshWorkflowProductionWorkspace.mockResolvedValue(EMPTY_WORKSPACE);
  serviceMocks.listGenerationCatalog.mockResolvedValue([recipe]);
  serviceMocks.getProjectWorkflowConfig.mockResolvedValue(EMPTY_PROJECT_CONFIG);
  serviceMocks.replaceProjectWorkflowConfig.mockResolvedValue(EMPTY_PROJECT_CONFIG);
}

interface WorkflowProjectClosureHarnessProps {
  onCatalogChanged: () => void;
  onOpenStudio: (workflowId: string, recipeId: string) => void;
  onUseInProject: (workflowId: string, recipeId: string) => void;
}

function WorkflowProjectClosureHarness({ onCatalogChanged, onOpenStudio, onUseInProject }: WorkflowProjectClosureHarnessProps) {
  const [catalog, setCatalog] = useState<RecipeViewModel[]>([]);
  const [route, setRoute] = useState<"workflows" | "project-settings">("workflows");

  async function refreshCatalog() {
    onCatalogChanged();
    setCatalog(await serviceMocks.listGenerationCatalog());
  }

  async function openStudio(workflowId: string, recipeId: string) {
    onOpenStudio(workflowId, recipeId);
  }

  async function useInProject(workflowId: string, recipeId: string) {
    onUseInProject(workflowId, recipeId);
    const nextCatalog = await serviceMocks.listGenerationCatalog();
    const exactRecipe = nextCatalog.find((candidate: RecipeViewModel) => candidate.workflowId === workflowId && candidate.recipeId === recipeId);
    if (!exactRecipe) return;
    setCatalog(nextCatalog);
    setRoute("project-settings");
  }

  if (route === "project-settings") {
    return <ProjectWorkflowSettings projectId="project-1" catalog={catalog} />;
  }

  return (
    <WorkflowWorkspace
      projectId="project-1"
      catalog={catalog}
      comfyConnected={false}
      onCatalogChanged={refreshCatalog}
      onOpenStudio={openStudio}
      onUseInProject={useInProject}
    />
  );
}

describe("DEV-079 添加工作流前端 UAT", () => {
  it("API 自动添加成功后显示用户结果和三类后续动作", async () => {
    const user = userEvent.setup();
    const useInProject = vi.fn();
    const openStudio = vi.fn();
    const returnToList = vi.fn();

    render(
      <WorkflowSmartImport
        plan={plan()}
        projectId="project-1"
        {...smartImportProps()}
        onUseInProject={useInProject}
        onOpenStudio={openStudio}
        onReturnToList={returnToList}
      />,
    );

    expect(screen.getByRole("heading", { name: "✓ 工作流已添加" })).toBeTruthy();
    expect(screen.getByText("Demo Workflow")).toBeTruthy();
    expect(screen.getByText("用途")).toBeTruthy();
    expect(screen.getByRole("button", { name: "用于当前项目" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "打开生成页面" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "用于当前项目" }));
    await user.click(screen.getByRole("button", { name: "打开生成页面" }));
    await user.click(screen.getByRole("button", { name: "返回工作流列表" }));
    expect(useInProject).toHaveBeenCalledWith("workflow-1", "recipe-1");
    expect(openStudio).toHaveBeenCalledWith("workflow-1", "recipe-1");
    expect(returnToList).toHaveBeenCalledTimes(1);
  });

  it("识别 UI JSON 时只显示导出指引，不进入发布成功态", async () => {
    const user = userEvent.setup();
    const props = smartImportProps();
    const retry = vi.fn();

    expect(workflowImportFormat(plan({ format: "UI", state: "BLOCKED" }))).toBe("UI");
    render(
      <WorkflowSmartImport
        plan={plan({ format: "UI", state: "BLOCKED" })}
        {...props}
        onRetry={retry}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByText("检测到 ComfyUI 普通工作流 JSON")).toBeTruthy();
    expect(screen.getByText("请在 ComfyUI 中将该工作流导出为 API Format JSON，然后重新选择该文件。")).toBeTruthy();
    expect(screen.queryByText("✓ 工作流已添加")).toBeNull();
    await user.click(screen.getByRole("button", { name: "选择另一个文件" }));
    expect(retry).toHaveBeenCalledTimes(1);
  });

  it("非法 JSON 和未知 JSON 都停留在未添加态", () => {
    const onRetry = vi.fn();
    const onCancel = vi.fn();
    const { rerender } = render(
      <WorkflowImportFormatIssue
        issue={{ kind: "INVALID_JSON", message: "无法读取这个文件，它不是有效的 JSON。" }}
        loading={false}
        onRetry={onRetry}
        onCancel={onCancel}
      />,
    );
    expect(screen.getByText("无法读取这个文件，它不是有效的 JSON。")).toBeTruthy();

    rerender(
      <WorkflowImportFormatIssue
        issue={{ kind: "UNKNOWN_FORMAT", message: "这个 JSON 不是可识别的 ComfyUI 工作流。" }}
        loading={false}
        onRetry={onRetry}
        onCancel={onCancel}
      />,
    );
    expect(screen.getByText("这个 JSON 不是可识别的 ComfyUI 工作流。")).toBeTruthy();
    expect(screen.queryByText("✓ 工作流已添加")).toBeNull();
  });

  it("非格式导入异常显示可展开的详细原因，而不是裸 IMPORT_FAILED", async () => {
    const user = userEvent.setup();
    serviceMocks.autoOnboardWorkflow.mockRejectedValue({
      code: "IMPORT_FAILED",
      message: "graph inference could not resolve duration source",
    });
    serviceMocks.listWorkflowProductionWorkspace.mockResolvedValue(EMPTY_WORKSPACE);

    render(
      <WorkflowWorkspace
        projectId="project-1"
        catalog={[]}
        comfyConnected={false}
        onCatalogChanged={vi.fn().mockResolvedValue(undefined)}
        onOpenStudio={vi.fn().mockResolvedValue(undefined)}
        onUseInProject={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    await user.click(screen.getByRole("button", { name: "+ 添加工作流" }));
    expect(await screen.findByRole("heading", { name: "工作流未能导入" })).toBeTruthy();
    expect(screen.getByText("工作流导入未完成，请查看详细原因后重试。")).toBeTruthy();
    expect(screen.getByText("查看详细原因")).toBeTruthy();
    expect(screen.getByText("graph inference could not resolve duration source")).toBeTruthy();
  });

  it("真实 WorkflowWorkspace 添加后刷新 Catalog，打开生成页面不触发用于当前项目", async () => {
    const user = userEvent.setup();
    const recipe = addedRecipe("image");
    prepareTauriBoundary(recipe);
    const onCatalogChanged = vi.fn();
    const onOpenStudio = vi.fn();
    const onUseInProject = vi.fn();

    render(
      <WorkflowProjectClosureHarness
        onCatalogChanged={onCatalogChanged}
        onOpenStudio={onOpenStudio}
        onUseInProject={onUseInProject}
      />,
    );

    await user.click(screen.getByRole("button", { name: "+ 添加工作流" }));
    await screen.findByRole("heading", { name: "✓ 工作流已添加" });
    await waitFor(() => expect(serviceMocks.autoOnboardWorkflow).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(onCatalogChanged).toHaveBeenCalledTimes(1));
    expect(screen.getByText(recipe.name)).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "打开生成页面" }));
    expect(onOpenStudio).toHaveBeenCalledWith(recipe.workflowId, recipe.recipeId);
    expect(onOpenStudio).toHaveBeenCalledTimes(1);
    expect(onUseInProject).not.toHaveBeenCalled();
  });

  it.each([
    ["image", "图片默认工作流", "IMAGE"],
    ["video", "视频默认工作流", "VIDEO"],
  ] as const)("真实添加闭环将 %s 工作流带入项目设置并保存精确 Recipe identity", async (kind, selectLabel, stage) => {
    const user = userEvent.setup();
    const recipe = addedRecipe(kind);
    prepareTauriBoundary(recipe);
    const onCatalogChanged = vi.fn();
    const onOpenStudio = vi.fn();
    const onUseInProject = vi.fn();

    render(
      <WorkflowProjectClosureHarness
        onCatalogChanged={onCatalogChanged}
        onOpenStudio={onOpenStudio}
        onUseInProject={onUseInProject}
      />,
    );

    await user.click(screen.getByRole("button", { name: "+ 添加工作流" }));
    await screen.findByRole("heading", { name: "✓ 工作流已添加" });
    await waitFor(() => expect(onCatalogChanged).toHaveBeenCalledTimes(1));

    await user.click(screen.getByRole("button", { name: "用于当前项目" }));
    await screen.findByText("项目工作流设置");
    expect(onUseInProject).toHaveBeenCalledWith(recipe.workflowId, recipe.recipeId);
    expect(onUseInProject).toHaveBeenCalledTimes(1);
    expect(onOpenStudio).not.toHaveBeenCalled();
    expect(serviceMocks.listGenerationCatalog).toHaveBeenCalledTimes(2);

    const select = await screen.findByLabelText(selectLabel);
    expect(within(select).getByRole("option", { name: new RegExp(`${recipe.workflowVersionId}.*${recipe.recipeId}`) })).toBeTruthy();
    await user.selectOptions(select, `${recipe.workflowVersionId}:${recipe.recipeId}`);
    expect(serviceMocks.replaceProjectWorkflowConfig).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "保存工作流配置" }));
    await waitFor(() => expect(serviceMocks.replaceProjectWorkflowConfig).toHaveBeenCalledWith("project-1", {
      bindings: [{
        stage,
        mode: "DEFAULT",
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
      }],
    }));
  });

  it("高级输入映射允许 linked target，并明确执行时覆盖连接输入", async () => {
    const user = userEvent.setup();
    const recipe = addedRecipe("video");
    const reviewPlan = plan({
      state: "NEEDS_REVIEW",
      published: undefined,
      autoPublishable: false,
      message: "工作流需要确认",
      issues: [{
        code: "AMBIGUOUS_DURATION_SOURCE",
        field: "duration_seconds",
        message: "无法自动确认视频时长来源",
        candidates: [{ label: "节点 49 · value", nodeId: "49", inputName: "value", fieldType: "number" }],
      }],
    });
    const linkedDraft = {
      ...onboardingDraft(recipe),
      nodes: [{
        nodeId: "63",
        classType: "MiniMaxH3ReferenceToVideo",
        title: "视频生成",
        isOutputNode: true,
        inputs: [{
          name: "width",
          kind: "link",
          currentValueSummary: "节点 61 · 1",
          isLinked: true,
          bindable: false,
          suggestedType: "integer",
          suggestedSemanticKey: "width",
          numericMin: "16",
          numericMax: "2048",
          numericStep: "1",
          allowedOptions: [],
        }],
      }],
    };
    serviceMocks.autoOnboardWorkflow.mockResolvedValue(reviewPlan);
    serviceMocks.getOnboardingDraft.mockResolvedValue(linkedDraft);
    serviceMocks.setOnboardingInputMapping.mockResolvedValue(linkedDraft);
    serviceMocks.listWorkflowProductionWorkspace.mockResolvedValue(EMPTY_WORKSPACE);

    render(
      <WorkflowWorkspace
        projectId="project-1"
        catalog={[]}
        comfyConnected={false}
        onCatalogChanged={vi.fn().mockResolvedValue(undefined)}
        onOpenStudio={vi.fn().mockResolvedValue(undefined)}
        onUseInProject={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    await user.click(screen.getByRole("button", { name: "+ 添加工作流" }));
    await screen.findByRole("heading", { name: "需要确认后添加" });
    await user.click(screen.getByRole("button", { name: "高级编辑" }));
    await user.click(screen.getByRole("tab", { name: "输入映射" }));

    expect(screen.getByRole("button", { name: "确认映射" })).toBeTruthy();
    expect(screen.getByText(/此参数在执行时会覆盖当前节点连接输入/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "确认映射" }));
    await waitFor(() => expect(serviceMocks.setOnboardingInputMapping).toHaveBeenCalledWith(
      "dev079-p1-draft",
      expect.objectContaining({ targetNode: "63", targetInput: "width", defaultValue: undefined }),
    ));
  });
});
