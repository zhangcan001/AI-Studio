// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type { ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import { ProjectWorkflowSettings } from "./ProjectWorkflowSettings";

const mocks = vi.hoisted(() => ({
  getProjectWorkflowConfig: vi.fn(),
  replaceProjectWorkflowConfig: vi.fn(),
}));

vi.mock("../../services/tauriClient", () => ({
  getProjectWorkflowConfig: mocks.getProjectWorkflowConfig,
  replaceProjectWorkflowConfig: mocks.replaceProjectWorkflowConfig,
}));

const emptyConfig: ProjectWorkflowConfigView = {
  projectId: "project-1",
  videoModeOverrides: [],
};

const catalog: RecipeViewModel[] = [
  {
    workflowId: "image-workflow",
    workflowVersionId: "image-version",
    recipeId: "image-recipe",
    name: "图片工作流",
    category: "image",
    mode: "text_to_image",
    fields: [],
    outputTypes: ["image"],
  },
  {
    workflowId: "video-workflow",
    workflowVersionId: "video-version",
    recipeId: "video-recipe",
    name: "视频工作流",
    category: "video",
    mode: "text_to_video",
    fields: [{ key: "prompt", type: "textarea", label: "提示词", required: true, default: "" }],
    outputTypes: ["video"],
  },
  {
    workflowId: "i2v-workflow",
    workflowVersionId: "i2v-version",
    recipeId: "i2v-recipe",
    name: "图生视频工作流",
    category: "video",
    mode: "image_to_video",
    fields: [
      { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
      { key: "first_frame", type: "image", label: "首帧", required: true },
    ],
    outputTypes: ["video"],
  },
];

describe("ProjectWorkflowSettings", () => {
  afterEach(cleanup);

  beforeEach(() => {
    localStorage.clear();
    mocks.getProjectWorkflowConfig.mockReset().mockResolvedValue(emptyConfig);
    mocks.replaceProjectWorkflowConfig.mockReset().mockResolvedValue(emptyConfig);
  });

  it("offers a one-click legacy import only while the database config is empty", async () => {
    localStorage.setItem(
      "aistudio.selectedWorkflow.project-1.image",
      JSON.stringify({ workflowVersionId: "image-version", recipeId: "image-recipe" }),
    );
    render(<ProjectWorkflowSettings projectId="project-1" catalog={catalog} />);

    expect(await screen.findByText("检测到旧版本保存的工作流选择")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "导入旧设置" }));

    expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledWith("project-1", {
      bindings: [{
        stage: "IMAGE",
        mode: "DEFAULT",
        workflowVersionId: "image-version",
        recipeId: "image-recipe",
      }],
    });
  });

  it("shows formal controls and does not offer legacy import when the database is configured", async () => {
    mocks.getProjectWorkflowConfig.mockResolvedValue({
      ...emptyConfig,
      imageDefault: {
        stage: "IMAGE",
        mode: "DEFAULT",
        workflowVersionId: "image-version",
        recipeId: "image-recipe",
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
        available: true,
      },
    });
    render(<ProjectWorkflowSettings projectId="project-1" catalog={catalog} />);

    expect(await screen.findByLabelText("图片默认工作流")).toBeTruthy();
    expect(screen.queryByText("检测到旧版本保存的工作流选择")).toBeNull();
    expect(screen.getByText("项目工作流设置")).toBeTruthy();
  });

  it("keeps edits local until one explicit save replaces the complete config", async () => {
    render(<ProjectWorkflowSettings projectId="project-1" catalog={catalog} />);

    await screen.findByLabelText("图片默认工作流");
    await userEvent.selectOptions(screen.getByLabelText("图片默认工作流"), "image-version:image-recipe");
    await userEvent.selectOptions(screen.getByLabelText("视频默认工作流"), "video-version:video-recipe");
    await userEvent.selectOptions(screen.getByLabelText("图生视频"), "i2v-version:i2v-recipe");

    expect(mocks.replaceProjectWorkflowConfig).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "保存工作流配置" }));

    expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledWith("project-1", {
      bindings: [
        { stage: "IMAGE", mode: "DEFAULT", workflowVersionId: "image-version", recipeId: "image-recipe" },
        { stage: "VIDEO", mode: "DEFAULT", workflowVersionId: "video-version", recipeId: "video-recipe" },
        { stage: "VIDEO", mode: "FL2VA_IMAGE_TO_VIDEO", workflowVersionId: "i2v-version", recipeId: "i2v-recipe" },
      ],
    });
  });

  it("shows original stale IDs and only clears the binding after save", async () => {
    mocks.getProjectWorkflowConfig.mockResolvedValue({
      ...emptyConfig,
      imageDefault: {
        stage: "IMAGE",
        mode: "DEFAULT",
        workflowVersionId: "stale-version",
        recipeId: "stale-recipe",
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
        available: false,
      },
    });
    render(<ProjectWorkflowSettings projectId="project-1" catalog={catalog} />);

    expect(await screen.findByText(/原 WorkflowVersion：stale-version/)).toBeTruthy();
    expect(mocks.replaceProjectWorkflowConfig).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "清除绑定" }));
    await userEvent.click(screen.getByRole("button", { name: "保存工作流配置" }));

    expect(mocks.replaceProjectWorkflowConfig).toHaveBeenCalledWith("project-1", { bindings: [] });
  });
});
