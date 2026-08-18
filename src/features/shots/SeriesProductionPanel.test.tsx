import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ShotView } from "../../types/shot";
import type { SeriesProductionPlan } from "../../types/seriesProduction";
import {
  SeriesProductionPanel,
  SeriesPrepareResultView,
  matchesSeriesFilter,
  productionSeriesOptions,
  seriesEpisodeSceneIds,
  seriesEpisodeShotIds,
  seriesPrepareConfirmation,
  seriesPresetConfirmation,
  seriesProductionActionDisabled,
} from "./SeriesProductionPanel";

const tree: ProductionStructureTree = {
  projectId: "project-1",
  unassignedShotIds: [],
  series: [{
    id: "series-1", projectId: "project-1", ordinal: 0, name: "第一季", description: "", createdAt: "", updatedAt: "",
    episodes: [
      { id: "episode-2", seriesId: "series-1", ordinal: 1, name: "夜战", description: "", createdAt: "", updatedAt: "", scenes: [{ id: "scene-2", episodeId: "episode-2", ordinal: 0, name: "屋顶", description: "", shotIds: ["shot-3"], createdAt: "", updatedAt: "" }] },
      { id: "episode-1", seriesId: "series-1", ordinal: 0, name: "雨夜", description: "", createdAt: "", updatedAt: "", scenes: [{ id: "scene-1", episodeId: "episode-1", ordinal: 0, name: "巷口", description: "", shotIds: ["shot-2", "shot-1"], createdAt: "", updatedAt: "" }] },
    ],
  }],
};

const shot = (id: string, ordinal: number): ShotView => ({ id, projectId: "project-1", ordinal, name: id, promptText: "", createdAt: "", updatedAt: "", status: "DRAFT", imageStatus: "DRAFT", videoStatus: "DRAFT", stageConfigs: [], referenceAssets: [], generationLinks: [] });

const plan: SeriesProductionPlan = {
  projectId: "project-1", seriesId: "series-1", seriesName: "第一季", seriesOrdinal: 0, stage: "image", episodeTotal: 2, sceneTotal: 2, shotTotal: 3, done: 1, prepared: 1, eligible: 1, blocked: 1, readyEpisodeCount: 1, blockedEpisodeCount: 1, completedEpisodeCount: 0, canPrepareAll: false,
  episodes: [
    { episodeId: "episode-1", episodeName: "雨夜", episodeOrdinal: 0, sceneTotal: 1, shotTotal: 2, done: 1, prepared: 1, eligible: 0, blocked: 0, classification: "PREPARED", canPrepare: false, existingBatchIds: ["batch-1"], blockingReasons: [] },
    { episodeId: "episode-2", episodeName: "夜战", episodeOrdinal: 1, sceneTotal: 1, shotTotal: 1, done: 0, prepared: 0, eligible: 1, blocked: 1, classification: "PARTIAL", canPrepare: true, existingBatchIds: [], blockingReasons: ["缺少图片 Prompt"] },
  ],
};

describe("SeriesProductionPanel", () => {
  it("renders the series/stage selectors, summary, episode table, strict default and safety copy", () => {
    const html = renderToStaticMarkup(<SeriesProductionPanel projectId="project-1" tree={tree} shots={[shot("shot-1", 0), shot("shot-2", 1), shot("shot-3", 2)]} initialPlan={plan} />);
    expect(html).toContain("Series 选择");
    expect(html).toContain("图片");
    expect(html).toContain("第一季");
    expect(html).toContain("Ready Episodes");
    expect(html).toContain("全选可准备");
    expect(html).toContain("跳过阻塞内容，仅准备当前可生产场景");
    expect(html).toContain("不会自动启动 GPU");
    expect(html).toContain("雨夜");
    expect(html).toContain("缺少图片 Prompt");
  });

  it("keeps series scope ordered by global shot ordinal and filters episode classifications", () => {
    expect(productionSeriesOptions(tree)[0]).toEqual({ value: "series-1", label: "第01季 · 第一季" });
    expect(seriesEpisodeSceneIds(tree, ["episode-2", "episode-1"])).toEqual(["scene-1", "scene-2"]);
    expect(seriesEpisodeShotIds(tree, [shot("shot-3", 2), shot("shot-1", 0), shot("shot-2", 1)], ["episode-1", "episode-2"])).toEqual(["shot-1", "shot-2", "shot-3"]);
    expect(matchesSeriesFilter(plan.episodes[1], "ready")).toBe(true);
    expect(matchesSeriesFilter(plan.episodes[1], "blocked")).toBe(true);
    expect(matchesSeriesFilter(plan.episodes[0], "prepared")).toBe(true);
  });

  it("keeps strict mode, confirmation, busy lock and preservation copy explicit", () => {
    expect(seriesPresetConfirmation("第一季", 4, 32, 286, "image")).toContain("覆盖第一季中 4 集、32 个场景、286 个镜头的图片阶段配置");
    expect(seriesPresetConfirmation("第一季", 4, 32, 286, "image")).toContain("引用素材和已选媒体不会改变");
    expect(seriesPrepareConfirmation("第一季", 4, 27, "image")).toContain("预计最多创建 27 个 READY 批次");
    expect(seriesPrepareConfirmation("第一季", 4, 27, "image")).toContain("不会自动启动 GPU");
    expect(seriesProductionActionDisabled("prepare")).toBe(true);
    expect(seriesProductionActionDisabled(undefined)).toBe(false);
  });

  it("renders prepare result and queue/episode deep-link affordances", () => {
    const html = renderToStaticMarkup(<SeriesPrepareResultView result={{ projectId: "project-1", seriesId: "series-1", stage: "image", status: "PARTIAL", requestedEpisodes: 2, requestedScenes: 2, createdBatches: 1, createdItems: 3, alreadyPreparedEpisodes: [], skippedDoneEpisodes: [], skippedEmptyEpisodes: [], skippedBlockedEpisodes: ["episode-2"], episodeResults: [{ episodeId: "episode-2", episodeName: "夜战", status: "BLOCKED", createdBatches: 0, createdItems: 0, alreadyPrepared: false, skipped: true, blockingReasons: ["缺少 Prompt"], batchIds: [] }] }} onOpenProductionQueue={() => undefined} disabled={false} onNavigateToEpisode={() => undefined} />);
    expect(html).toContain("PARTIAL");
    expect(html).toContain("创建：<strong>1</strong> 个 Batch");
    expect(html).toContain("打开生产队列");
    expect(html).toContain("查看集");
  });
});
