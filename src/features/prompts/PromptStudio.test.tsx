// @vitest-environment jsdom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ModelVersionView, ModelView } from "../../types/model";
import type { PromptEntryView } from "../../types/prompt";
import { PromptStudio } from "./PromptStudio";

const mocks = vi.hoisted(() => ({
  listPromptLibrary: vi.fn(),
  getPromptLibraryEntry: vi.fn(),
  listModels: vi.fn(),
  listModelVersions: vi.fn(),
  listGenerationAssetVersionLinks: vi.fn(),
  listGenerationToolUsages: vi.fn(),
  taskHistoryPage: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, ...mocks };
});

const promptListItem: PromptEntryView = {
  id: "prm-1",
  projectId: "project-1",
  kind: "prompt",
  name: "人物镜头提示词",
  tags: ["人物"],
  createdAt: "2026-09-01T00:00:00Z",
  updatedAt: "2026-09-02T00:00:00Z",
  versionCount: 2,
  versions: [],
};

const promptDetail: PromptEntryView = {
  ...promptListItem,
  versions: [
    { id: "prv-1", promptId: "prm-1", version: 1, text: "旧版人物镜头", createdAt: "2026-09-01T00:00:00Z", modelVersionId: null },
    { id: "prv-2", promptId: "prm-1", version: 2, text: "电影感人物近景，柔和侧光", createdAt: "2026-09-02T00:00:00Z", modelVersionId: "mdv-1" },
  ],
};

const model: ModelView = {
  id: "mdl-1",
  name: "H3",
  provider: "MiniMax",
  type: "video",
  description: "视频模型",
  metadata: { local: true },
  createdAt: "2026-09-01T00:00:00Z",
};

const modelVersion: ModelVersionView = {
  id: "mdv-1",
  modelId: "mdl-1",
  version: "2.0",
  capabilities: ["text_to_video"],
  parameterSchema: { fps: { type: "integer" } },
  createdAt: "2026-09-01T00:00:00Z",
};

describe("PromptStudio", () => {
  beforeEach(() => {
    mocks.listPromptLibrary.mockResolvedValue({ items: [promptListItem], nextCursor: undefined });
    mocks.getPromptLibraryEntry.mockResolvedValue(promptDetail);
    mocks.listModels.mockResolvedValue([model]);
    mocks.listModelVersions.mockResolvedValue([modelVersion]);
    mocks.listGenerationAssetVersionLinks.mockResolvedValue([]);
    mocks.listGenerationToolUsages.mockResolvedValue([]);
    mocks.taskHistoryPage.mockResolvedValue({
      items: [{
        id: "tsk-1",
        workflowId: "workflow-1",
        workflowVersionId: "workflow-version-1",
        recipeId: "recipe-1",
        workflowName: "视频生成",
        status: "SUCCEEDED",
        createdAt: "2026-09-02T01:00:00Z",
        outputCount: 1,
      }],
      workflowOptions: [],
    });
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("renders the project-scoped prompt list with required metadata columns", async () => {
    render(<PromptStudio projectId="project-1" />);

    expect(await screen.findByRole("button", { name: "人物镜头提示词" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "名称" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "类型" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "当前版本" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "模型" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "更新时间" })).toBeTruthy();
    expect(within(screen.getByRole("region", { name: "提示词列表" })).getByText("MiniMax / H3 · 2.0")).toBeTruthy();
    expect(mocks.listPromptLibrary).toHaveBeenCalledWith("project-1", expect.objectContaining({ kind: "prompt" }));
    expect(mocks.getPromptLibraryEntry).toHaveBeenCalledWith("project-1", "prm-1");
    expect(mocks.taskHistoryPage).toHaveBeenCalledWith(expect.objectContaining({ projectId: "project-1" }));
  });

  it("shows prompt detail, model view, version history, and provenance", async () => {
    const user = userEvent.setup();
    render(<PromptStudio projectId="project-1" />);

    expect(await screen.findByText("电影感人物近景，柔和侧光")).toBeTruthy();
    expect(screen.getAllByText("参数规范").length).toBeGreaterThan(0);
    expect(screen.getByText("参考素材")).toBeTruthy();
    expect(screen.getByText("生成历史")).toBeTruthy();
    expect(screen.getByText("视频生成")).toBeTruthy();
    expect(screen.getByText("生成快照")).toBeTruthy();
    expect(screen.getByText("结果资产")).toBeTruthy();
    expect(screen.getByText("模型视图")).toBeTruthy();
    expect(screen.getByText("能力")).toBeTruthy();
    expect(screen.getByRole("region", { name: "模型视图" }).textContent).toContain("text_to_video");

    const versionHistory = screen.getByRole("navigation", { name: "提示词版本历史" });
    await user.click(within(versionHistory).getByRole("button", { name: /v1/ }));
    expect(screen.getByText("旧版人物镜头")).toBeTruthy();
  });

  it("shows explicit cross-module provenance and missing-link states", async () => {
    mocks.listGenerationToolUsages.mockResolvedValue([{
      id: "gtu-1",
      generationId: "tsk-1",
      toolInstanceId: "tins-comfy",
      toolVersionId: "tver-comfy-1",
      metadata: { source: "explicit" },
      createdAt: "2026-09-02T01:00:00Z",
    }]);
    mocks.listGenerationAssetVersionLinks.mockResolvedValue([{
      id: "gav-1",
      generationId: "tsk-1",
      outputId: "output-0",
      ordinal: 0,
      assetVersionId: "av-1",
      relationType: "OUTPUT",
      createdAt: "2026-09-02T01:01:00Z",
    }]);

    render(<PromptStudio projectId="project-1" />);

    expect(await screen.findByText("Used Generations")).toBeTruthy();
    expect(screen.getByText("tver-comfy-1")).toBeTruthy();
    expect(screen.getByText("av-1")).toBeTruthy();
    expect(screen.getByText("当前数据层尚未建立 Prompt Version → Generation 显式关系；以下仅为当前项目任务历史，不推断为当前提示词直接使用。")).toBeTruthy();
    expect(mocks.listGenerationToolUsages).toHaveBeenCalledWith("project-1", "tsk-1");
    expect(mocks.listGenerationAssetVersionLinks).toHaveBeenCalledWith("project-1", "tsk-1");
  });

  it("reports an empty explicit provenance state for historical generations", async () => {
    render(<PromptStudio projectId="project-1" />);

    expect(await screen.findByText("暂无显式工具使用记录；历史生成可能未保存工具关系。")).toBeTruthy();
    expect(screen.getByText("任务有输出，但尚未建立 Generation → AssetVersion 显式关系。")).toBeTruthy();
  });

  it("uses the typed filter transport", async () => {
    const user = userEvent.setup();
    render(<PromptStudio projectId="project-1" />);

    await screen.findByRole("button", { name: "人物镜头提示词" });
    await user.selectOptions(screen.getByLabelText("提示词类型"), "snippet");
    await waitFor(() => expect(mocks.listPromptLibrary).toHaveBeenCalledWith("project-1", expect.objectContaining({ kind: "snippet" })));
  });

  it("exposes loading and list error states", async () => {
    mocks.listPromptLibrary.mockImplementationOnce(() => new Promise(() => undefined));
    render(<PromptStudio projectId="project-1" />);
    expect(screen.getByText("正在加载提示词…")).toBeTruthy();

    cleanup();
    mocks.listPromptLibrary.mockRejectedValueOnce(new Error("list unavailable"));
    render(<PromptStudio projectId="project-1" />);
    expect(await screen.findByText("提示词列表加载失败：操作失败，请查看技术详情。")).toBeTruthy();
  });

  it("exposes a detail error when a prompt cannot be read", async () => {
    mocks.getPromptLibraryEntry.mockRejectedValue(new Error("detail unavailable"));
    render(<PromptStudio projectId="project-1" />);
    expect(await screen.findByText("提示词详情加载失败：操作失败，请查看技术详情。")).toBeTruthy();
  });

  it("exposes model and generation-history errors", async () => {
    mocks.listModels.mockRejectedValueOnce(new Error("model registry unavailable"));
    mocks.taskHistoryPage.mockRejectedValueOnce(new Error("history unavailable"));
    render(<PromptStudio projectId="project-1" />);
    expect(await screen.findByText("模型加载失败：操作失败，请查看技术详情。")).toBeTruthy();
    expect(await screen.findByText("生成历史加载失败：操作失败，请查看技术详情。")).toBeTruthy();
  });

  it("exposes an empty state", async () => {
    mocks.listPromptLibrary.mockResolvedValue({ items: [], nextCursor: undefined });
    render(<PromptStudio projectId="project-2" />);
    expect(await screen.findByText("当前项目暂无提示词。")).toBeTruthy();
  });
});
