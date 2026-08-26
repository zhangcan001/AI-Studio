import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
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
    expect(html).toContain("READY");
    expect(html).toContain("INCOMPLETE");
    expect(html).toContain("BLOCKED");
    expect(html).toContain("已准备");
    expect(html).toContain("选择全部 READY");
    expect(html).toContain("已有旧上下文准备版本");
    expect(html).toContain("ComfyUI 离线");
    expect(html).not.toContain("立即启动");
    expect(html).not.toContain("开始全部");
    expect(html).not.toContain("启动生产");
  });

  it("keeps admission capped at 100 and excludes incomplete, blocked, and already-prepared shots", () => {
    expect(MAX_PREPARATION_BATCH_ITEMS).toBe(100);
    expect(preparationSelectionLimit(500)).toBe(100);
    expect(preparationCanSelect(summary())).toBe(true);
    expect(preparationCanSelect(summary({ status: "INCOMPLETE" }))).toBe(false);
    expect(preparationCanSelect(summary({ status: "BLOCKED" }))).toBe(false);
    expect(preparationCanSelect(summary({ alreadyPrepared: true }))).toBe(false);
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
});
