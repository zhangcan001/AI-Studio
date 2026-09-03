// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeField, RecipeViewModel } from "../../types/generation";
import type {
  ComfyPreflightIssue,
  ComfyPreflightReport,
  ComfyPreflightWorkflowItem,
} from "../../types/settings";
import type { ProjectWorkflowBindingView, ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import { KERA2_WORKFLOW_ID } from "../runtime/productRuntimeScope";
import { ProjectProductionReadiness } from "./ProjectProductionReadiness";
import { ProjectWorkflowSettings } from "./ProjectWorkflowSettings";

const mocks = vi.hoisted(() => ({
  getProjectWorkflowConfig: vi.fn(),
  replaceProjectWorkflowConfig: vi.fn(),
  getComfyPreflight: vi.fn(),
}));

vi.mock("../../services/tauriClient", () => ({
  getProjectWorkflowConfig: mocks.getProjectWorkflowConfig,
  replaceProjectWorkflowConfig: mocks.replaceProjectWorkflowConfig,
  getComfyPreflight: mocks.getComfyPreflight,
}));

function promptField(): RecipeField {
  return { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" };
}

function imageRecipe(id: string): RecipeViewModel {
  return {
    workflowId: KERA2_WORKFLOW_ID,
    workflowVersionId: `image-${id}`,
    recipeId: `image-recipe-${id}`,
    name: `Image ${id}`,
    category: "image",
    mode: "text_to_image",
    fields: [promptField()],
    outputTypes: ["image"],
  };
}

function videoRecipe(id: string): RecipeViewModel {
  return {
    workflowId: `video-workflow-${id}`,
    workflowVersionId: `video-${id}`,
    recipeId: `video-recipe-${id}`,
    name: `Video ${id}`,
    category: "video",
    mode: "video",
    fields: [
      promptField(),
      { key: "first_frame", type: "image", label: "First", required: false },
      { key: "last_frame", type: "image", label: "Last", required: false },
      { key: "reference_image", type: "image", label: "Reference image", required: false },
      { key: "reference_video", type: "video", label: "Reference video", required: false },
      { key: "reference_audio", type: "audio", label: "Reference audio", required: false },
    ],
    outputTypes: ["video"],
  };
}

function binding(recipe: RecipeViewModel, stage: "IMAGE" | "VIDEO"): ProjectWorkflowBindingView {
  return {
    stage,
    mode: "DEFAULT",
    workflowVersionId: recipe.workflowVersionId,
    recipeId: recipe.recipeId,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    available: true,
  };
}

function config(patch: Partial<ProjectWorkflowConfigView> = {}): ProjectWorkflowConfigView {
  return { projectId: "project-uat", videoModeOverrides: [], ...patch };
}

function runtimeItem(
  recipe: RecipeViewModel,
  status = "READY",
  patch: Partial<ComfyPreflightWorkflowItem> = {},
): ComfyPreflightWorkflowItem {
  return {
    workflowId: recipe.workflowId,
    workflowVersionId: recipe.workflowVersionId,
    name: recipe.name,
    version: "1",
    status,
    missingNodes: [],
    reason: null,
    ...patch,
  };
}

function issue(patch: Partial<ComfyPreflightIssue> = {}): ComfyPreflightIssue {
  return {
    severity: "ERROR",
    code: "WORKFLOW_BLOCKED",
    title: "工作流问题",
    detail: "当前工作流存在问题。",
    workflowId: null,
    workflowVersionId: null,
    missingNodes: null,
    suggestedAction: null,
    ...patch,
  };
}

function runtimeReport(
  items: ComfyPreflightWorkflowItem[],
  patch: Partial<ComfyPreflightReport> = {},
): ComfyPreflightReport {
  return {
    endpoint: "http://127.0.0.1:8188",
    status: "READY",
    checkedAt: "2026-09-03T10:00:00Z",
    connection: "CONNECTED",
    comfyuiVersion: "0.33.0",
    pythonVersion: "3.12.10",
    gpu: "NVIDIA Test GPU",
    vramTotal: 16 * 1024 ** 3,
    vramFree: 8 * 1024 ** 3,
    nodeCount: 4516,
    runtimeBusy: false,
    activeTaskCount: 0,
    productionBusy: false,
    workflowSummary: {
      workflowTotal: items.length,
      workflowReady: items.filter((item) => item.status === "READY").length,
      workflowBlocked: items.filter((item) => item.status === "BLOCKED").length,
      items,
    },
    issues: [],
    ...patch,
  };
}

const IMAGE_A = imageRecipe("a");
const IMAGE_B = imageRecipe("b");
const VIDEO_A = videoRecipe("a");
const CATALOG = [IMAGE_A, IMAGE_B, VIDEO_A];
const READY_RUNTIME = runtimeReport([runtimeItem(IMAGE_A), runtimeItem(VIDEO_A)]);
const IMAGE_SHARED_A = { ...IMAGE_A, workflowVersionId: "image-shared", recipeId: "image-recipe-a" };
const IMAGE_SHARED_B = { ...IMAGE_B, workflowVersionId: "image-shared", recipeId: "image-recipe-b" };
const SHARED_RECIPE_CATALOG = [IMAGE_SHARED_A, IMAGE_SHARED_B, VIDEO_A];
const READY_SHARED_RUNTIME = runtimeReport([runtimeItem(IMAGE_SHARED_A), runtimeItem(VIDEO_A)]);

function Harness({ initialConfig, catalog = CATALOG }: { initialConfig: ProjectWorkflowConfigView; catalog?: RecipeViewModel[] }) {
  const [currentConfig, setCurrentConfig] = useState(initialConfig);
  return (
    <>
      <ProjectWorkflowSettings
        projectId={initialConfig.projectId}
        catalog={catalog}
        onConfigChanged={setCurrentConfig}
      />
      <ProjectProductionReadiness config={currentConfig} catalog={catalog} />
    </>
  );
}

describe("ProjectProductionReadiness deterministic UAT", () => {
  beforeEach(() => {
    mocks.getProjectWorkflowConfig.mockReset();
    mocks.replaceProjectWorkflowConfig.mockReset();
    mocks.getComfyPreflight.mockReset();
  });

  afterEach(cleanup);

  it("Case 1: checks an image default against a connected idle runtime", async () => {
    const initialConfig = config({ imageDefault: binding(IMAGE_A, "IMAGE") });
    mocks.getProjectWorkflowConfig.mockResolvedValue(initialConfig);
    mocks.getComfyPreflight.mockResolvedValue(READY_RUNTIME);
    render(<Harness initialConfig={initialConfig} />);

    await screen.findByLabelText("图片默认工作流");
    await userEvent.click(screen.getByRole("button", { name: "检查开工条件" }));

    await waitFor(() => expect(screen.getByText("✓ 项目可以开工")).toBeTruthy());
    expect(screen.getByText(/WorkflowVersion：image-a/)).toBeTruthy();
    expect(mocks.getComfyPreflight).toHaveBeenCalledTimes(1);
  });

  it("Case 2: reports missing nodes for a blocked video runtime workflow", async () => {
    const initialConfig = config({ videoDefault: binding(VIDEO_A, "VIDEO") });
    mocks.getProjectWorkflowConfig.mockResolvedValue(initialConfig);
    mocks.getComfyPreflight.mockResolvedValue(runtimeReport([
      runtimeItem(IMAGE_A),
      runtimeItem(VIDEO_A, "BLOCKED", { missingNodes: ["NodeX"], reason: "缺少节点 NodeX" }),
    ], { status: "BLOCKED" }));
    render(<Harness initialConfig={initialConfig} />);

    await screen.findByLabelText("图片默认工作流");
    await userEvent.click(screen.getByRole("button", { name: "检查开工条件" }));

    await waitFor(() => expect(screen.getAllByText(/NodeX/).length).toBeGreaterThan(0));
    expect(screen.getAllByText("Runtime：BLOCKED").length).toBeGreaterThan(0);
  });

  it("Case 3: reports BUSY while leaving the check read-only", async () => {
    const initialConfig = config();
    mocks.getProjectWorkflowConfig.mockResolvedValue(initialConfig);
    mocks.getComfyPreflight.mockResolvedValue(runtimeReport([runtimeItem(IMAGE_A), runtimeItem(VIDEO_A)], {
      runtimeBusy: true,
      activeTaskCount: 1,
      productionBusy: true,
    }));
    render(<Harness initialConfig={initialConfig} />);

    await screen.findByLabelText("图片默认工作流");
    await userEvent.click(screen.getByRole("button", { name: "检查开工条件" }));

    await waitFor(() => expect(screen.getByText("⏳ 运行环境忙碌")).toBeTruthy());
    expect(mocks.replaceProjectWorkflowConfig).not.toHaveBeenCalled();
  });

  it("Case 4: invalidates the old runtime snapshot after saving A to B", async () => {
    const initialConfig = config({ imageDefault: binding(IMAGE_A, "IMAGE") });
    const nextConfig = config({ imageDefault: binding(IMAGE_B, "IMAGE") });
    mocks.getProjectWorkflowConfig.mockResolvedValue(initialConfig);
    mocks.replaceProjectWorkflowConfig.mockResolvedValue(nextConfig);
    mocks.getComfyPreflight.mockResolvedValue(READY_RUNTIME);
    render(<Harness initialConfig={initialConfig} />);

    const user = userEvent.setup();
    await screen.findByLabelText("图片默认工作流");
    await user.click(screen.getByRole("button", { name: "检查开工条件" }));
    await waitFor(() => expect(screen.getByText(/WorkflowVersion：image-a/)).toBeTruthy());
    await user.selectOptions(screen.getByLabelText("图片默认工作流"), "image-b:image-recipe-b");
    await user.click(screen.getByRole("button", { name: "保存工作流配置" }));

    await waitFor(() => expect(screen.getByText("项目工作流已变化，请重新检查开工条件。")).toBeTruthy());
    expect(screen.getByText("尚未检查当前运行环境")).toBeTruthy();
    expect(mocks.getComfyPreflight).toHaveBeenCalledTimes(1);
  });

  it("Case 7: invalidates after ProjectWorkflowSettings saves recipe B with the same workflow version", async () => {
    const initialConfig = config({ imageDefault: binding(IMAGE_SHARED_A, "IMAGE") });
    const nextConfig = config({ imageDefault: binding(IMAGE_SHARED_B, "IMAGE") });
    mocks.getProjectWorkflowConfig.mockResolvedValue(initialConfig);
    mocks.replaceProjectWorkflowConfig.mockResolvedValue(nextConfig);
    mocks.getComfyPreflight.mockResolvedValue(READY_SHARED_RUNTIME);
    render(<Harness initialConfig={initialConfig} catalog={SHARED_RECIPE_CATALOG} />);

    const user = userEvent.setup();
    await screen.findByLabelText("图片默认工作流");
    await user.click(screen.getByRole("button", { name: "检查开工条件" }));
    await waitFor(() => expect(screen.getByText(/WorkflowVersion：image-shared/)).toBeTruthy());
    await user.selectOptions(screen.getByLabelText("图片默认工作流"), "image-shared:image-recipe-b");
    await user.click(screen.getByRole("button", { name: "保存工作流配置" }));

    await waitFor(() => expect(screen.getByText("项目工作流已变化，请重新检查开工条件。")).toBeTruthy());
    expect(screen.getByText("尚未检查当前运行环境")).toBeTruthy();
    expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledWith("project-uat", {
      bindings: [{
        stage: "IMAGE",
        mode: "DEFAULT",
        workflowVersionId: "image-shared",
        recipeId: "image-recipe-b",
      }],
    });
    expect(mocks.getComfyPreflight).toHaveBeenCalledTimes(1);
  });

  it("Case 5: ignores an unrelated blocked workflow", async () => {
    const initialConfig = config();
    mocks.getProjectWorkflowConfig.mockResolvedValue(initialConfig);
    mocks.getComfyPreflight.mockResolvedValue(runtimeReport([runtimeItem(IMAGE_A), runtimeItem(VIDEO_A)], {
      status: "BLOCKED",
      issues: [issue({ code: "UNRELATED", workflowId: "workflow-z", workflowVersionId: "version-z" })],
    }));
    render(<Harness initialConfig={initialConfig} />);

    await screen.findByLabelText("图片默认工作流");
    await userEvent.click(screen.getByRole("button", { name: "检查开工条件" }));

    await waitFor(() => expect(screen.getByText("✓ 项目可以开工")).toBeTruthy());
    expect(screen.queryByText("UNRELATED")).toBeNull();
  });

  it("Case 6: maps a degraded runtime workflow to a runnable warning", async () => {
    const initialConfig = config();
    mocks.getProjectWorkflowConfig.mockResolvedValue(initialConfig);
    mocks.getComfyPreflight.mockResolvedValue(runtimeReport([
      runtimeItem(IMAGE_A),
      runtimeItem(VIDEO_A, "DEGRADED", { reason: "尚未完成真实生成验证" }),
    ]));
    render(<Harness initialConfig={initialConfig} />);

    await screen.findByLabelText("图片默认工作流");
    await userEvent.click(screen.getByRole("button", { name: "检查开工条件" }));

    await waitFor(() => expect(screen.getAllByText(/可开工，但需要注意/).length).toBeGreaterThan(0));
    expect(screen.getAllByText(/尚未完成真实生成验证/).length).toBeGreaterThan(0);
  });
});
