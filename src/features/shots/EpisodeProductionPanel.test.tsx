import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { getEpisodeProductionPlan, prepareEpisodeProduction } from "../../services/tauriClient";
import type { EpisodeProductionPlan } from "../../types/episodeProduction";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ShotView } from "../../types/shot";
import {
  EpisodeProductionPanel,
  EpisodePrepareResultView,
  episodePrepareConfirmation,
  episodePresetConfirmation,
  episodeProductionActionDisabled,
  episodeSceneShotIds,
  matchesEpisodeFilter,
  productionEpisodeOptions,
} from "./EpisodeProductionPanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const tree: ProductionStructureTree = {
  projectId: "project-1", unassignedShotIds: [], series: [{
    id: "series-1", projectId: "project-1", ordinal: 0, name: "第一季", description: "", createdAt: "", updatedAt: "", episodes: [{
      id: "episode-1", seriesId: "series-1", ordinal: 2, name: "夜战", description: "", createdAt: "", updatedAt: "", scenes: [
        { id: "scene-1", episodeId: "episode-1", ordinal: 1, name: "巷口", description: "", shotIds: ["shot-2", "shot-1"], createdAt: "", updatedAt: "" },
        { id: "scene-2", episodeId: "episode-1", ordinal: 2, name: "屋顶", description: "", shotIds: ["shot-3"], createdAt: "", updatedAt: "" },
      ],
    }],
  }],
};

const shot = (id: string, ordinal: number): ShotView => ({ id, projectId: "project-1", ordinal, name: id, promptText: "", createdAt: "", updatedAt: "", status: "DRAFT", imageStatus: "DRAFT", videoStatus: "DRAFT", stageConfigs: [], referenceAssets: [], generationLinks: [] });

const plan: EpisodeProductionPlan = {
  projectId: "project-1", seriesId: "series-1", seriesName: "第一季", episodeId: "episode-1", episodeName: "夜战", episodeOrdinal: 2, stage: "image", sceneTotal: 2, shotTotal: 3, done: 1, prepared: 1, eligible: 1, blocked: 1, readySceneCount: 1, blockedSceneCount: 1, fullyDoneSceneCount: 0, canPrepareAll: false,
  scenes: [
    { sceneId: "scene-1", sceneName: "巷口", sceneOrdinal: 1, total: 2, done: 1, prepared: 1, eligible: 0, blocked: 0, canPrepare: false, classification: "PREPARED", existingBatchIds: ["batch-1"], blockingReasons: [] },
    { sceneId: "scene-2", sceneName: "屋顶", sceneOrdinal: 2, total: 1, done: 0, prepared: 0, eligible: 1, blocked: 1, canPrepare: true, classification: "PARTIAL", existingBatchIds: [], blockingReasons: ["缺少图片 Prompt"] },
  ],
};

describe("EpisodeProductionPanel", () => {
  it("renders Episode/stage selectors, summary, filters, scene rows, strict default and safe prepare copy", () => {
    const html = renderToStaticMarkup(<EpisodeProductionPanel projectId="project-1" tree={tree} shots={[shot("shot-1", 0), shot("shot-2", 1), shot("shot-3", 2)]} initialPlan={plan} />);
    expect(html).toContain("Episode 选择");
    expect(html).toContain("图片");
    expect(html).toContain("第一季");
    expect(html).toContain("夜战");
    expect(html).toContain("全选可准备");
    expect(html).toContain("全选有阻塞");
    expect(html).toContain("跳过阻塞场景，仅准备当前可生产内容");
    expect(html).toContain("不会自动启动 GPU");
    expect(html).toContain("巷口");
    expect(html).toContain("缺少图片 Prompt");
  });

  it("keeps scene scope ordered and filters classifications in the frontend", () => {
    expect(productionEpisodeOptions(tree)[0]).toEqual({ value: "episode-1", label: "第一季 / 第03集 · 夜战", seriesName: "第一季" });
    expect(episodeSceneShotIds(tree, [shot("shot-1", 0), shot("shot-2", 1), shot("shot-3", 2)], ["scene-1"])).toEqual(["shot-1", "shot-2"]);
    expect(matchesEpisodeFilter(plan.scenes[1], "ready")).toBe(true);
    expect(matchesEpisodeFilter(plan.scenes[1], "blocked")).toBe(true);
    expect(matchesEpisodeFilter(plan.scenes[0], "prepared")).toBe(true);
  });

  it("states confirmation and busy safety contracts", () => {
    expect(episodePresetConfirmation("image", 8, 73)).toContain("覆盖 8 个场景、73 个镜头的图片阶段配置");
    expect(episodePresetConfirmation("image", 8, 73)).toContain("引用素材和已选媒体不会改变");
    expect(episodePrepareConfirmation("夜战", "image", 6, 47)).toContain("不会自动启动 GPU");
    expect(episodeProductionActionDisabled("prepare")).toBe(true);
    expect(episodeProductionActionDisabled(undefined)).toBe(false);
  });

  it("renders SUCCESS, PARTIAL, BLOCKED result states, queue navigation and scene deep links", () => {
    const onOpenQueue = vi.fn();
    const onNavigate = vi.fn();
    const successHtml = renderToStaticMarkup(<EpisodePrepareResultView result={{ projectId: "project-1", episodeId: "episode-1", stage: "image", status: "SUCCESS", requestedScenes: 1, createdBatches: 1, createdItems: 2, alreadyPreparedScenes: [], skippedDoneScenes: [], skippedEmptyScenes: [], skippedBlockedScenes: [], results: [] }} onOpenProductionQueue={onOpenQueue} disabled={false} onNavigateToScene={onNavigate} />);
    const partialHtml = renderToStaticMarkup(<EpisodePrepareResultView result={{ projectId: "project-1", episodeId: "episode-1", stage: "image", status: "PARTIAL", requestedScenes: 2, createdBatches: 1, createdItems: 2, alreadyPreparedScenes: [], skippedDoneScenes: [], skippedEmptyScenes: [], skippedBlockedScenes: ["scene-2"], results: [{ sceneId: "scene-2", sceneName: "屋顶", status: "FAILED", created: false, createdCount: 0, existingBatchIds: [], blockingReasons: [], error: "状态发生变化" }] }} onOpenProductionQueue={onOpenQueue} disabled={false} onNavigateToScene={onNavigate} />);
    const blockedHtml = renderToStaticMarkup(<EpisodePrepareResultView result={{ projectId: "project-1", episodeId: "episode-1", stage: "image", status: "BLOCKED", requestedScenes: 1, createdBatches: 0, createdItems: 0, alreadyPreparedScenes: [], skippedDoneScenes: [], skippedEmptyScenes: [], skippedBlockedScenes: ["scene-2"], results: [{ sceneId: "scene-2", sceneName: "屋顶", status: "BLOCKED", created: false, createdCount: 0, existingBatchIds: [], blockingReasons: ["缺少 Prompt"] }] }} onOpenProductionQueue={onOpenQueue} disabled={false} onNavigateToScene={onNavigate} />);
    expect(successHtml).toContain("SUCCESS");
    expect(successHtml).toContain("打开生产队列");
    expect(partialHtml).toContain("PARTIAL");
    expect(partialHtml).toContain("状态发生变化");
    expect(blockedHtml).toContain("BLOCKED");
    expect(blockedHtml).toContain("查看场景");
  });
});

describe("Episode production client contracts", () => {
  it("uses the assumed request envelope for Agent A commands", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(plan);
    await getEpisodeProductionPlan({ projectId: "project-1", episodeId: "episode-1", stage: "image" });
    expect(invoke).toHaveBeenLastCalledWith("episode_production_plan", { request: { projectId: "project-1", episodeId: "episode-1", stage: "image" } });

    vi.mocked(invoke).mockResolvedValueOnce({ ...plan, status: "SUCCESS", requestedScenes: 1, createdBatches: 1, createdItems: 2, alreadyPreparedScenes: [], skippedDoneScenes: [], skippedEmptyScenes: [], skippedBlockedScenes: [], results: [] });
    await prepareEpisodeProduction({ projectId: "project-1", episodeId: "episode-1", stage: "image", sceneIds: ["scene-1"], allowPartial: false });
    expect(invoke).toHaveBeenLastCalledWith("episode_production_prepare", { request: { projectId: "project-1", episodeId: "episode-1", stage: "image", sceneIds: ["scene-1"], allowPartial: false } });
  });
});
