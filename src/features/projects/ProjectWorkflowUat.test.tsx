// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeField, RecipeViewModel } from "../../types/generation";
import type {
  ProjectWorkflowBindingView,
  ProjectWorkflowConfigView,
  ProjectWorkflowMode,
} from "../../types/projectWorkflow";
import { KERA2_WORKFLOW_ID, MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID } from "../runtime/productRuntimeScope";
import { resolveProjectVideoWorkflow } from "../runtime/projectWorkflowResolution";
import { ProjectWorkflowPreflight } from "./ProjectWorkflowPreflight";
import { ProjectWorkflowSettings } from "./ProjectWorkflowSettings";

const mocks = vi.hoisted(() => ({
  getProjectWorkflowConfig: vi.fn(),
  replaceProjectWorkflowConfig: vi.fn(),
}));

vi.mock("../../services/tauriClient", () => ({
  getProjectWorkflowConfig: mocks.getProjectWorkflowConfig,
  replaceProjectWorkflowConfig: mocks.replaceProjectWorkflowConfig,
}));

const PROJECT_ID = "project-uat";
const TIMESTAMP = "2026-01-01T00:00:00Z";

function promptField(): RecipeField {
  return { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" };
}

function mediaField(key: string): RecipeField {
  const type = key.includes("audio") ? "audio" : key.includes("video") ? "video" : "image";
  return { key, type, label: key, required: false };
}

function imageRecipe(id: string): RecipeViewModel {
  return {
    workflowId: KERA2_WORKFLOW_ID,
    workflowVersionId: `image-${id}-version`,
    recipeId: `image-${id}-recipe`,
    name: `Image ${id}`,
    category: "image",
    mode: "text_to_image",
    fields: [
      promptField(),
      { key: "width", type: "integer", label: "Width", required: true, default: 1024 },
      { key: "height", type: "integer", label: "Height", required: true, default: 1024 },
      { key: "seed", type: "seed", label: "Seed", defaultMode: "random" },
    ],
    outputTypes: ["image"],
  };
}

function videoRecipe(id: string, mediaKeys: string[], workflowId = `video-${id}`): RecipeViewModel {
  return {
    workflowId,
    workflowVersionId: `video-${id}-version`,
    recipeId: `video-${id}-recipe`,
    name: `Video ${id}`,
    category: "video",
    mode: "video",
    fields: [promptField(), ...mediaKeys.map(mediaField)],
    outputTypes: ["video"],
  };
}

function genericVideoRecipe(): RecipeViewModel {
  return {
    workflowId: "generic-custom-video",
    workflowVersionId: "generic-video-version",
    recipeId: "generic-video-recipe",
    name: "Generic Custom Video",
    category: "video",
    mode: "video",
    fields: [],
    outputTypes: ["video"],
  };
}

const IMAGE_A = imageRecipe("a");
const IMAGE_B = imageRecipe("b");
const VIDEO_A = videoRecipe("a", ["first_frame", "last_frame"]);
const VIDEO_B = videoRecipe("b", ["reference_image", "reference_audio", "reference_video"], MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID);
const FULL_CATALOG = [IMAGE_A, IMAGE_B, VIDEO_A, VIDEO_B];

function binding(
  stage: "IMAGE" | "VIDEO",
  mode: ProjectWorkflowMode,
  recipe: Pick<RecipeViewModel, "workflowVersionId" | "recipeId">,
  available = true,
): ProjectWorkflowBindingView {
  return {
    stage,
    mode,
    workflowVersionId: recipe.workflowVersionId,
    recipeId: recipe.recipeId,
    createdAt: TIMESTAMP,
    updatedAt: TIMESTAMP,
    available,
  };
}

function staleBinding(stage: "IMAGE" | "VIDEO", mode: ProjectWorkflowMode): ProjectWorkflowBindingView {
  return {
    stage,
    mode,
    workflowVersionId: "stale-video-version",
    recipeId: "stale-video-recipe",
    createdAt: TIMESTAMP,
    updatedAt: TIMESTAMP,
    available: false,
  };
}

function config(patch: Partial<ProjectWorkflowConfigView> = {}): ProjectWorkflowConfigView {
  return { projectId: PROJECT_ID, videoModeOverrides: [], ...patch };
}

function ProjectWorkflowUatHarness({
  initialConfig,
  catalog = FULL_CATALOG,
}: {
  initialConfig: ProjectWorkflowConfigView;
  catalog?: RecipeViewModel[];
}) {
  const [currentConfig, setCurrentConfig] = useState(initialConfig);
  return (
    <>
      <ProjectWorkflowSettings
        projectId={PROJECT_ID}
        catalog={catalog}
        onConfigChanged={setCurrentConfig}
      />
      <ProjectWorkflowPreflight config={currentConfig} catalog={catalog} />
    </>
  );
}

function recipeValue(recipe: Pick<RecipeViewModel, "workflowVersionId" | "recipeId">): string {
  return `${recipe.workflowVersionId}:${recipe.recipeId}`;
}

async function renderUat(initialConfig: ProjectWorkflowConfigView, catalog = FULL_CATALOG) {
  mocks.getProjectWorkflowConfig.mockResolvedValue(initialConfig);
  render(<ProjectWorkflowUatHarness initialConfig={initialConfig} catalog={catalog} />);
  await screen.findByLabelText("图片默认工作流");
}

function itemForPath(label: string): HTMLElement {
  const item = screen.getAllByTestId("project-workflow-preflight-item").find((candidate) => (
    within(candidate).queryByText(label, { exact: true })
  ));
  if (!item) throw new Error(`Preflight path not found: ${label}`);
  return item;
}

describe("Project Workflow deterministic UI UAT", () => {
  beforeEach(() => {
    localStorage.clear();
    mocks.getProjectWorkflowConfig.mockReset();
    mocks.replaceProjectWorkflowConfig.mockReset();
  });

  afterEach(cleanup);

  it("Case A: treats an empty project config as production-ready when the catalog is compatible", async () => {
    await renderUat(config());

    expect(screen.getAllByTestId("project-workflow-preflight-item")).toHaveLength(8);
    expect(screen.getByText("✓ 项目工作流可生产")).toBeTruthy();
    expect(itemForPath("图片生成").textContent).toContain("WorkflowVersion：image-a-version");
    expect(itemForPath("图片生成").textContent).toContain("来源：系统推荐");
    expect(itemForPath("文生视频").textContent).toContain("来源：兼容回退");
    expect(itemForPath("参考音频视频").textContent).toContain("来源：系统推荐");
  });

  it("Case B: saves an image default and updates the real preflight panel", async () => {
    const nextConfig = config({ imageDefault: binding("IMAGE", "DEFAULT", IMAGE_A) });
    mocks.replaceProjectWorkflowConfig.mockResolvedValue(nextConfig);
    await renderUat(config());

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText("图片默认工作流"), recipeValue(IMAGE_A));
    await user.click(screen.getByRole("button", { name: "保存工作流配置" }));

    await waitFor(() => expect(itemForPath("图片生成").textContent).toContain("来源：项目图片默认"));
    expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledTimes(1);
    expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledWith(PROJECT_ID, {
      bindings: [{
        stage: "IMAGE",
        mode: "DEFAULT",
        workflowVersionId: IMAGE_A.workflowVersionId,
        recipeId: IMAGE_A.recipeId,
      }],
    });
  });

  it("Case C: keeps a valid video default only for modes it actually supports", async () => {
    await renderUat(config({ videoDefault: binding("VIDEO", "DEFAULT", VIDEO_A) }));

    expect(itemForPath("文生视频").textContent).toContain("WorkflowVersion：video-a-version");
    expect(itemForPath("图生视频").textContent).toContain("WorkflowVersion：video-a-version");
    expect(itemForPath("首尾帧视频").textContent).toContain("WorkflowVersion：video-a-version");

    const referenceAudio = itemForPath("参考音频视频");
    expect(referenceAudio.textContent).toContain("WorkflowVersion：video-b-version");
    expect(referenceAudio.textContent).not.toContain("WorkflowVersion：video-a-version");
    expect(referenceAudio.textContent).toContain("来源：系统推荐");
  });

  it("Case D: resolves a mode override before the project video default", async () => {
    await renderUat(config({
      videoDefault: binding("VIDEO", "DEFAULT", VIDEO_A),
      videoModeOverrides: [binding("VIDEO", "FL2VA_IMAGE_TO_VIDEO", VIDEO_B)],
    }));

    const textToVideo = itemForPath("文生视频");
    const imageToVideo = itemForPath("图生视频");
    expect(textToVideo.textContent).toContain("WorkflowVersion：video-a-version");
    expect(textToVideo.textContent).toContain("来源：项目视频默认");
    expect(imageToVideo.textContent).toContain("WorkflowVersion：video-b-version");
    expect(imageToVideo.textContent).toContain("来源：模式专用");
  });

  it("Case E: warns on a stale override, falls back visibly, and does not persist anything", async () => {
    await renderUat(config({
      videoDefault: binding("VIDEO", "DEFAULT", VIDEO_A),
      videoModeOverrides: [staleBinding("VIDEO", "FL2VA_IMAGE_TO_VIDEO")],
    }));

    const imageToVideo = itemForPath("图生视频");
    expect(imageToVideo.textContent).toContain("需要注意");
    expect(imageToVideo.textContent).toContain("WorkflowVersion：video-a-version");
    expect(imageToVideo.textContent).toContain("来源：项目视频默认");
    expect(imageToVideo.textContent).toContain("原 WorkflowVersion：stale-video-version");
    expect(imageToVideo.textContent).toContain("原 Recipe：stale-video-recipe");
    expect(imageToVideo.textContent).toContain("建议重新选择或清除失效绑定");
    expect(mocks.replaceProjectWorkflowConfig).not.toHaveBeenCalled();
  });

  it("Case F: blocks an H3 mode without an exact recipe even when generic CUSTOM_VIDEO exists", async () => {
    const generic = genericVideoRecipe();
    await renderUat(config(), [IMAGE_A, generic]);

    expect(screen.getByText("⚠ 项目工作流部分可用")).toBeTruthy();
    const imageToVideo = itemForPath("图生视频");
    expect(imageToVideo.textContent).toContain("✕ 当前无兼容工作流");
    expect(imageToVideo.textContent).not.toContain(generic.workflowVersionId);

    const strict = resolveProjectVideoWorkflow(
      [generic],
      "FL2VA_IMAGE_TO_VIDEO",
      undefined,
      undefined,
      undefined,
      undefined,
      { allowGenericFallback: false },
    );
    const legacy = resolveProjectVideoWorkflow(
      [generic],
      "FL2VA_IMAGE_TO_VIDEO",
      undefined,
      undefined,
      undefined,
      undefined,
      { allowGenericFallback: true },
    );
    expect(strict.recipe).toBeUndefined();
    expect(legacy.recipe).toBe(generic);
    expect(legacy.source).toBe("compatible");
  });

  it("Case G: converges preflight from image A to image B after one live save", async () => {
    const initialConfig = config({ imageDefault: binding("IMAGE", "DEFAULT", IMAGE_A) });
    const nextConfig = config({ imageDefault: binding("IMAGE", "DEFAULT", IMAGE_B) });
    mocks.replaceProjectWorkflowConfig.mockResolvedValue(nextConfig);
    await renderUat(initialConfig);

    const user = userEvent.setup();
    const preflightPanel = screen.getByRole("region", { name: "生产可用性" });
    expect(itemForPath("图片生成").textContent).toContain("WorkflowVersion：image-a-version");

    await user.selectOptions(screen.getByLabelText("图片默认工作流"), recipeValue(IMAGE_B));
    expect(mocks.replaceProjectWorkflowConfig).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "保存工作流配置" }));

    await waitFor(() => expect(itemForPath("图片生成").textContent).toContain("WorkflowVersion：image-b-version"));
    expect(itemForPath("图片生成").textContent).toContain("来源：项目图片默认");
    expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("region", { name: "生产可用性" })).toBe(preflightPanel);
  });
});
