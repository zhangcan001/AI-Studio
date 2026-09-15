// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TaskDetail } from "../../types/history";
import { TaskHistoryDetail } from "./TaskHistoryDetail";

const mocks = vi.hoisted(() => ({
  listGenerationAssetVersionLinks: vi.fn(),
  listGenerationToolUsages: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, ...mocks };
});

const detail: TaskDetail = {
  id: "tsk-1",
  projectId: "project-1",
  workflowId: "workflow-1",
  workflowVersionId: "workflow-version-1",
  recipeId: "recipe-1",
  workflowName: "视频生成",
  status: "SUCCEEDED",
  createdAt: "2026-09-02T01:00:00Z",
  finishedAt: "2026-09-02T01:02:00Z",
  outputAssets: [],
  reusableDraft: { available: false, missingAssetIds: [] },
};

function renderDetail() {
  return render(
    <TaskHistoryDetail
      projectId="project-1"
      detail={detail}
      loadingDraft={false}
      comfyConnected={true}
      productionBusy={false}
      onBack={vi.fn()}
      onLoadInputs={vi.fn()}
      onOpenAsset={vi.fn()}
    />,
  );
}

describe("TaskHistoryDetail provenance visibility", () => {
  beforeEach(() => {
    mocks.listGenerationToolUsages.mockResolvedValue([{
      id: "gtu-1",
      generationId: "tsk-1",
      toolInstanceId: "tins-comfy",
      toolVersionId: "tver-comfy-1",
      metadata: { source: "explicit" },
      createdAt: "2026-09-02T01:00:30Z",
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
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("renders tool usage, asset-version lineage, and the timeline", async () => {
    renderDetail();

    expect(await screen.findByRole("region", { name: "生成溯源" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "工具使用" }).textContent).toContain("tver-comfy-1");
    expect(screen.getByRole("region", { name: "资产版本" }).textContent).toContain("av-1");
    expect(screen.getByRole("region", { name: "溯源时间线" }).textContent).toContain("来源任务");
    expect(screen.getByRole("region", { name: "溯源时间线" }).textContent).toContain("工具使用");
    expect(screen.getByRole("region", { name: "溯源时间线" }).textContent).toContain("资产版本");
    expect(mocks.listGenerationToolUsages).toHaveBeenCalledWith("project-1", "tsk-1");
    expect(mocks.listGenerationAssetVersionLinks).toHaveBeenCalledWith("project-1", "tsk-1");
  });

  it("shows an explicit empty state when no historical links exist", async () => {
    mocks.listGenerationToolUsages.mockResolvedValue([]);
    mocks.listGenerationAssetVersionLinks.mockResolvedValue([]);
    renderDetail();

    expect(await screen.findByText("当前任务暂无显式跨模块关系；旧任务可能没有保存历史关联。")).toBeTruthy();
    expect(screen.getByText("暂无显式工具使用记录。")).toBeTruthy();
    expect(screen.getByText("暂无显式资产版本关系。")).toBeTruthy();
  });

  it("reports a lineage loading error without hiding the task detail", async () => {
    mocks.listGenerationToolUsages.mockRejectedValueOnce(new Error("lineage unavailable"));
    renderDetail();

    expect(await screen.findByText("生成溯源加载失败：操作失败，请查看技术详情。")).toBeTruthy();
    expect(screen.getByText("视频生成")).toBeTruthy();
  });
});
