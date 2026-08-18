import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { ProductionBatchRunbookView } from "../../types/productionBatchRunbook";
import {
  ProductionBatchRunbookPanel,
  canStartRunbookRow,
  filterRunbookRows,
  runbookProgress,
  sortRunbookRows,
} from "./ProductionBatchRunbookPanel";

const row = (overrides: Partial<ProductionBatchRunbookView["rows"][number]> = {}): ProductionBatchRunbookView["rows"][number] => ({
  batchId: "batch-image", batchName: "第一季 · 第01集 · 场景01 · 图片", batchStatus: "READY", stage: "image", seriesId: "series-1", seriesName: "第一季", seriesOrdinal: 0, episodeId: "episode-1", episodeName: "雨夜", episodeOrdinal: 0, sceneId: "scene-1", sceneName: "巷口", sceneOrdinal: 0, shotCount: 10, pending: 10, active: 0, succeeded: 0, failed: 0, createdAt: "2026-08-18T00:00:00Z", readyToStart: true, blockedReason: null, mixedScope: false, ...overrides,
});

const runbook: ProductionBatchRunbookView = {
  projectId: "project-1", seriesId: "series-1", recommendedBatchId: "batch-image", recommendationReason: "第一个 READY 且通过 admission", rows: [
    row(),
    row({ batchId: "batch-running", batchName: "第一季 · 第01集 · 场景02 · 图片", batchStatus: "RUNNING", sceneId: "scene-2", sceneName: "屋顶", sceneOrdinal: 1, readyToStart: false, active: 2, pending: 3, succeeded: 5 }),
    row({ batchId: "batch-generic", batchName: "Generic", episodeId: "", episodeName: "", sceneId: "", sceneName: "", readyToStart: false }),
  ],
};

describe("ProductionBatchRunbookPanel", () => {
  it("renders hierarchy, running recommendation, recommended batch and deep links", () => {
    const html = renderToStaticMarkup(<ProductionBatchRunbookPanel projectId="project-1" runbook={runbook} onStartBatch={vi.fn()} onOpenProductionQueue={vi.fn()} onNavigateToScene={vi.fn()} onNavigateToEpisode={vi.fn()} />);
    expect(html).toContain("生产批次执行清单");
    expect(html).toContain("当前正在生产");
    expect(html).toContain("建议下一批");
    expect(html).toContain("打开队列");
    expect(html).toContain("巷口");
    expect(html).toContain("屋顶");
  });

  it("sorts by series/episode/scene/stage and supports default active filters", () => {
    const video = row({ batchId: "batch-video", stage: "video", createdAt: "2026-08-17T00:00:00Z" });
    const image = row({ batchId: "batch-image-late", stage: "image", createdAt: "2026-08-19T00:00:00Z" });
    expect(sortRunbookRows([video, image]).map((item) => item.batchId)).toEqual(["batch-image-late", "batch-video"]);
    expect(filterRunbookRows([row(), row({ batchId: "done", batchStatus: "COMPLETED" })], "active")).toHaveLength(2);
    expect(filterRunbookRows([row(), row({ batchId: "done", batchStatus: "COMPLETED" })], "ready")).toHaveLength(1);
  });

  it("allows one READY start only when admission is clear and flags mixed scope", () => {
    expect(canStartRunbookRow(row(), false)).toBe(true);
    expect(canStartRunbookRow(row(), true)).toBe(false);
    expect(canStartRunbookRow(row({ mixedScope: true }), false)).toBe(false);
    expect(canStartRunbookRow(row({ blockedReason: "当前队列被阻塞" }), false)).toBe(false);
    expect(runbookProgress(row({ succeeded: 5 }))).toBe(50);
    expect(runbookProgress(row({ shotCount: 0, succeeded: 0 }))).toBe(0);
  });
});
