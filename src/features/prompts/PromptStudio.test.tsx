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

  it("uses the typed filter transport", async () => {
    const user = userEvent.setup();
    render(<PromptStudio projectId="project-1" />);

    await screen.findByRole("button", { name: "人物镜头提示词" });
    await user.selectOptions(screen.getByLabelText("提示词类型"), "snippet");
    await waitFor(() => expect(mocks.listPromptLibrary).toHaveBeenCalledWith("project-1", expect.objectContaining({ kind: "snippet" })));
  });

  it("exposes an empty state", async () => {
    mocks.listPromptLibrary.mockResolvedValue({ items: [], nextCursor: undefined });
    render(<PromptStudio projectId="project-2" />);
    expect(await screen.findByText("当前项目暂无提示词。")).toBeTruthy();
  });
});
