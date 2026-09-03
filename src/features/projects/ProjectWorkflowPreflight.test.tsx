// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RecipeField, RecipeViewModel } from "../../types/generation";
import type { ProjectWorkflowBindingView, ProjectWorkflowConfigView, ProjectWorkflowMode } from "../../types/projectWorkflow";
import { KERA2_WORKFLOW_ID, MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID } from "../runtime/productRuntimeScope";
import { ProjectWorkflowPreflight } from "./ProjectWorkflowPreflight";

const mocks = vi.hoisted(() => ({
  getProjectWorkflowConfig: vi.fn(),
}));

vi.mock("../../services/tauriClient", () => ({
  getProjectWorkflowConfig: mocks.getProjectWorkflowConfig,
}));

function promptField(): RecipeField {
  return { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" };
}

function mediaField(key: string, type: "image" | "video" | "audio"): RecipeField {
  return { key, type, label: key, required: false };
}

function videoRecipe(id: string, mediaKeys: string[] = [], workflowId = `workflow-${id}`): RecipeViewModel {
  return {
    workflowId,
    workflowVersionId: `version-${id}`,
    recipeId: `recipe-${id}`,
    name: `Video ${id}`,
    category: "video",
    mode: "video",
    fields: [
      promptField(),
      ...mediaKeys.map((key) => mediaField(key, key.includes("audio") ? "audio" : key.includes("video") ? "video" : "image")),
    ],
    outputTypes: ["video"],
  };
}

function imageRecipe(): RecipeViewModel {
  return {
    workflowId: KERA2_WORKFLOW_ID,
    workflowVersionId: "version-image",
    recipeId: "recipe-image",
    name: "Krea Image",
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

function binding(stage: "IMAGE" | "VIDEO", mode: ProjectWorkflowMode, id: string, available = true): ProjectWorkflowBindingView {
  return {
    stage,
    mode,
    workflowVersionId: `version-${id}`,
    recipeId: `recipe-${id}`,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    available,
  };
}

function config(patch: Partial<ProjectWorkflowConfigView> = {}): ProjectWorkflowConfigView {
  return { projectId: "project-1", videoModeOverrides: [], ...patch };
}

const readyCatalog = [
  imageRecipe(),
  videoRecipe("default", ["first_frame"]),
  videoRecipe("override", ["first_frame"]),
  videoRecipe("recommended", ["reference_image", "reference_audio", "reference_video"], MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID),
  videoRecipe("compatible", ["first_frame", "last_frame"]),
];

describe("ProjectWorkflowPreflight", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("shows eight paths, the overall READY state, every source, and a stale warning", () => {
    const validOverride = binding("VIDEO", "FL2VA_IMAGE_TO_VIDEO", "override");
    const staleOverride = binding("VIDEO", "FL2VA_FIRST_LAST", "stale", false);
    render(
      <ProjectWorkflowPreflight
        config={config({
          imageDefault: binding("IMAGE", "DEFAULT", "image"),
          videoDefault: binding("VIDEO", "DEFAULT", "default"),
          videoModeOverrides: [validOverride, staleOverride],
        })}
        catalog={readyCatalog}
      />,
    );

    expect(screen.getAllByTestId("project-workflow-preflight-item")).toHaveLength(8);
    expect(screen.getByText("✓ 项目工作流可生产")).toBeTruthy();
    expect(screen.getByText(/来源：项目图片默认/)).toBeTruthy();
    expect(screen.getByText(/来源：项目视频默认/)).toBeTruthy();
    expect(screen.getByText(/来源：模式专用/)).toBeTruthy();
    expect(screen.getAllByText(/来源：系统推荐/).length).toBeGreaterThan(0);
    expect(screen.getByText(/来源：兼容回退/)).toBeTruthy();
    expect(screen.getByText(/项目绑定不可用，当前实际使用/)).toBeTruthy();
    expect(screen.getByText(/version-stale/)).toBeTruthy();
  });

  it("shows PARTIAL and BLOCKED path states", () => {
    const partial = render(
      <ProjectWorkflowPreflight config={config()} catalog={[imageRecipe(), videoRecipe("t2v")]} />,
    );
    expect(screen.getByText("⚠ 项目工作流部分可用")).toBeTruthy();
    expect(screen.getAllByText("✕ 当前无兼容工作流").length).toBeGreaterThan(0);
    partial.unmount();

    render(<ProjectWorkflowPreflight config={config()} catalog={[]} />);
    expect(screen.getByText("✕ 当前没有可用生产工作流")).toBeTruthy();
    expect(screen.getAllByText("✕ 当前无兼容工作流")).toHaveLength(8);
  });

  it("rechecks the current project configuration without remounting", async () => {
    const nextConfig = config({ videoDefault: binding("VIDEO", "DEFAULT", "override") });
    mocks.getProjectWorkflowConfig.mockResolvedValue(nextConfig);
    render(
      <ProjectWorkflowPreflight
        config={config({ videoDefault: binding("VIDEO", "DEFAULT", "default") })}
        catalog={readyCatalog}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "重新检查" }));

    await waitFor(() => expect(screen.getAllByText(/version-override/).length).toBeGreaterThan(0));
    expect(mocks.getProjectWorkflowConfig).toHaveBeenCalledWith("project-1");
  });
});
