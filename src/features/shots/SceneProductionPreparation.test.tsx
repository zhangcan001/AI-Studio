// @vitest-environment jsdom
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import * as tauriClient from "../../services/tauriClient";
import type {
  ScenePreparationView,
  ShotProductionPlanSummary,
} from "../../types/productionPreparation";
import {
  MAX_PREPARATION_BATCH_ITEMS,
  preparationCanSelect,
} from "../../types/productionPreparation";
import {
  SceneProductionPreparation,
  preparationSelectionLimit,
} from "./SceneProductionPreparation";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.mocked(invoke).mockReset(); });

const summary = (overrides: Partial<ShotProductionPlanSummary> = {}): ShotProductionPlanSummary => ({
  shotId: "shot-1",
  ordinal: 0,
  name: "雨夜入口",
  status: "READY",
  score: 95,
  warningCount: 1,
  incompleteCount: 0,
  blockerCount: 0,
  contextHash: "hash-shot-1",
  characterNames: ["主角"],
  characterCount: 1,
  sceneProfileName: "雨夜入口",
  referenceCount: 2,
  workflowVersionId: "workflow-1",
  recipeId: "recipe-1",
  currentStageStatus: "未开始",
  alreadyPrepared: false,
  existingBatchIds: [],
  matchingPreparedBatchIds: [],
  stalePreparedBatchIds: [],
  blockers: [],
  warnings: [],
  legacy: false,
  ...overrides,
});

const view: ScenePreparationView = {
  projectId: "project-1",
  sceneId: "scene-1",
  sceneName: "雨夜入口",
  stage: "image",
  total: 4,
  readyCount: 2,
  incompleteCount: 1,
  blockedCount: 1,
  preparedCount: 1,
  warningCount: 1,
  evaluatedAt: "2026-08-26T08:00:00Z",
  items: [
    summary(),
    summary({ shotId: "shot-2", ordinal: 1, name: "巷口近景", alreadyPrepared: true, matchingPreparedBatchIds: ["batch-1"], stalePreparedBatchIds: ["batch-old"] }),
    summary({ shotId: "shot-3", ordinal: 2, name: "屋檐切换", status: "INCOMPLETE", score: 70, warningCount: 0, incompleteCount: 1, blockerCount: 0, blockers: ["缺少视频关键帧"] }),
    summary({ shotId: "shot-4", ordinal: 3, name: "远景收束", status: "BLOCKED", score: 40, warningCount: 0, incompleteCount: 0, blockerCount: 1, blockers: ["ComfyUI 离线"] }),
  ],
};

describe("SceneProductionPreparation", () => {
  it("renders the scene preparation first screen with counts and only READY selection affordances", () => {
    const html = renderToStaticMarkup(
      <SceneProductionPreparation
        projectId="project-1"
        sceneOptions={[{ value: "scene-1", label: "S01 / 雨夜入口" }]}
        currentSceneId="scene-1"
        initialView={view}
      />,
    );

    expect(html).toContain("场景生产准备");
    expect(html).toContain("<span>总镜头</span><strong>4</strong>");
    expect(html).toContain("<span>可准备</span><strong>2</strong>");
    expect(html).toContain("<span>资料不完整</span><strong>1</strong>");
    expect(html).toContain("<span>有阻塞</span><strong>1</strong>");
    expect(html).toContain("<span>已准备</span><strong>1</strong>");
    expect(html).toContain("可准备");
    expect(html).toContain("资料不完整");
    expect(html).toContain("有阻塞");
    expect(html).toContain("已准备");
    expect(html).toContain("选择全部可准备");
    expect(html).toContain("已有旧上下文准备版本");
    expect(html).toContain("ComfyUI 离线");
    expect(html).not.toContain("立即启动");
    expect(html).not.toContain("开始全部");
    expect(html).not.toContain("启动生产");

    const checkboxes = html.match(/<input[^>]*type="checkbox"[^>]*>/g) ?? [];
    expect(checkboxes).toHaveLength(4);
    expect(checkboxes.find((markup) => markup.includes('aria-label="选择 雨夜入口"'))).not.toContain("disabled");
    expect(checkboxes.find((markup) => markup.includes('aria-label="选择 巷口近景"'))).toContain("disabled");
    expect(checkboxes.find((markup) => markup.includes('aria-label="选择 屋檐切换"'))).toContain("disabled");
    expect(checkboxes.find((markup) => markup.includes('aria-label="选择 远景收束"'))).toContain("disabled");
  });

  it("renders the 100-shot selection boundary and excludes incomplete, blocked, and already-prepared shots", () => {
    expect(MAX_PREPARATION_BATCH_ITEMS).toBe(100);
    expect(preparationSelectionLimit(500)).toBe(100);
    expect(preparationCanSelect(summary())).toBe(true);
    expect(preparationCanSelect(summary({ status: "INCOMPLETE" }))).toBe(false);
    expect(preparationCanSelect(summary({ status: "BLOCKED" }))).toBe(false);
    expect(preparationCanSelect(summary({ alreadyPrepared: true }))).toBe(false);

    const html = renderToStaticMarkup(
      <SceneProductionPreparation
        projectId="project-1"
        sceneOptions={[{ value: "scene-1", label: "S01 / 雨夜入口" }]}
        currentSceneId="scene-1"
        initialView={largeReadyView}
      />,
    );
    expect(html).toContain("可准备镜头超过 100 个");
    expect(html).toContain("只取前 100 个");
    expect(html.match(/type="checkbox"/g) ?? []).toHaveLength(105);
  });
});

describe("Scene preparation client boundary", () => {
  it("uses one preflight envelope and admission never calls startProductionQueue", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(view);
    await tauriClient.getSceneProductionPreflight({ projectId: "project-1", sceneId: "scene-1", stage: "image" });
    expect(invoke).toHaveBeenLastCalledWith("scene_production_preflight", {
      request: { projectId: "project-1", sceneId: "scene-1", stage: "image" },
    });

    const startSpy = vi.spyOn(tauriClient, "startProductionQueue");
    vi.mocked(invoke).mockResolvedValueOnce({
      projectId: "project-1",
      stage: "image",
      requestedCount: 1,
      createdCount: 1,
      skippedIncomplete: 0,
      skippedBlocked: 0,
      alreadyPreparedCount: 0,
      createdBatchIds: ["batch-1"],
      matchingPreparedBatchIds: [],
    });
    await tauriClient.admitSceneProduction({
      projectId: "project-1",
      sceneId: "scene-1",
      stage: "image",
      shotIds: ["shot-1"],
      allowPartial: false,
    });
    expect(invoke).toHaveBeenLastCalledWith("scene_production_admit", {
      request: {
        projectId: "project-1",
        sceneId: "scene-1",
        stage: "image",
        shotIds: ["shot-1"],
        allowPartial: false,
      },
    });
    expect(startSpy).not.toHaveBeenCalled();
    startSpy.mockRestore();
  });

  it("clicks Select all READY at 100, admits the selection, and navigates to Queue without starting it", async () => {
    const user = userEvent.setup();
    const videoView = { ...largeReadyView, stage: "video" as const };
    const admissionResult = {
      projectId: "project-1", sceneId: "scene-1", stage: "video" as const,
      batchId: "batch-100", createdBatchIds: ["batch-100"], createdCount: 100,
      skippedIncomplete: 0, skippedBlocked: 0, alreadyPreparedCount: 0,
      matchingPreparedBatchIds: [],
    };
    const onOpenProductionQueue = vi.fn();
    const startSpy = vi.spyOn(tauriClient, "startProductionQueue");
    vi.spyOn(window, "confirm").mockReturnValue(true);
    let admitted = false;
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "scene_production_preflight") return admitted
        ? { ...videoView, preparedCount: 100, items: videoView.items.map((item, index) => ({ ...item, alreadyPrepared: index < 100 })) }
        : videoView;
      if (command === "scene_production_admit") { admitted = true; return admissionResult; }
      throw new Error(`Unexpected IPC: ${command}`);
    });
    render(<SceneProductionPreparation projectId="project-1"
      sceneOptions={[{ value: "scene-1", label: "S01 / 雨夜入口" }]}
      currentSceneId="scene-1" onOpenProductionQueue={onOpenProductionQueue} />);
    await screen.findByRole("checkbox", { name: "选择 镜头 1" });
    await waitFor(() => expect(screen.getByRole("button", { name: "选择全部可准备" }).hasAttribute("disabled")).toBe(false));
    await user.click(screen.getByRole("button", { name: "选择全部可准备" }));
    expect(screen.getAllByRole("checkbox").filter(input => (input as HTMLInputElement).checked)).toHaveLength(100);
    await user.click(screen.getByRole("button", { name: "创建待启动批次" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("scene_production_admit", {
      request: { projectId: "project-1", sceneId: "scene-1", stage: "video",
        shotIds: expect.arrayContaining(["shot-1", "shot-100"]), allowPartial: false },
    }));
    const call = vi.mocked(invoke).mock.calls.find(([command]) => command === "scene_production_admit");
    expect((call?.[1] as { request: { shotIds: string[] } }).request.shotIds).toHaveLength(100);
    await screen.findByText(/已准备完成：/);
    await user.click(screen.getByRole("button", { name: "前往生产队列" }));
    expect(onOpenProductionQueue).toHaveBeenCalledWith("batch-100");
    expect(startSpy).not.toHaveBeenCalled();
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "start_production_queue")).toBe(false);
  });

  it("rechecks the scene and rejects a selection that has become blocked", async () => {
    const user = userEvent.setup();
    vi.spyOn(window, "confirm").mockReturnValue(true);
    vi.mocked(invoke).mockResolvedValueOnce({ ...view, stage: "video" })
      .mockResolvedValueOnce({ ...view, stage: "video", items: [summary({ status: "BLOCKED", blockers: ["首帧已经删除"] })] });
    render(<SceneProductionPreparation projectId="project-1" sceneOptions={[{ value: "scene-1", label: "Scene" }]} currentSceneId="scene-1" />);
    await screen.findByRole("checkbox", { name: "选择 雨夜入口" });
    await waitFor(() => expect(screen.getByRole("button", { name: "选择全部可准备" }).hasAttribute("disabled")).toBe(false));
    await user.click(screen.getByRole("button", { name: "选择全部可准备" }));
    await user.click(screen.getByRole("button", { name: "创建待启动批次" }));
    await screen.findByText(/最新预检未通过/);
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "scene_production_admit")).toHaveLength(0);
  });
});
const largeReadyView: ScenePreparationView = {
  ...view,
  total: 105,
  readyCount: 105,
  incompleteCount: 0,
  blockedCount: 0,
  preparedCount: 0,
  warningCount: 0,
  items: Array.from({ length: 105 }, (_, index) => summary({
    shotId: `shot-${index + 1}`,
    ordinal: index,
    name: `镜头 ${index + 1}`,
  })),
};
