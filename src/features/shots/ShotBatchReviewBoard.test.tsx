import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { AssetView } from "../../types/asset";
import type { ProductionReviewProductivityItem } from "../../services/tauriClient";
import { regenerateProductionItem } from "../../services/tauriClient";
import { ShotBatchReviewBoard, matchesFilter, reviewCounts, reviewImageIdsForItem, toCompareItem, toLocalCompareItem } from "./ShotBatchReviewBoard";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const asset = (id: string, kind: "image" | "video" = "image"): AssetView => ({
  id, assetType: kind, category: kind === "video" ? "generated_video" : "generated_image", name: id, originalName: id,
  mimeType: kind === "video" ? "video/mp4" : "image/png", fileSize: 1, createdAt: "2026-08-27T00:00:00Z", isFavorite: false, tags: [],
});

const reviewItem = (overrides: Partial<ProductionReviewProductivityItem> = {}): ProductionReviewProductivityItem => ({
  itemId: "item-1", ordinal: 0, taskId: "task-1", taskStatus: "SUCCEEDED", productionItemStatus: "SUCCEEDED", reviewStatus: "UNREVIEWED", reviewNote: "",
  preferred: false, workflowVersionId: "workflow-1", recipeId: "recipe-1", qualityProfile: "QUALITY", createdAt: "2026-08-27T00:00:00Z", outputAssets: [asset("asset-a")],
  shotId: "shot-1", stage: "IMAGE", selectedAssetId: undefined, reviewable: true,
  candidateAssets: [{ assetId: "asset-a", assetType: "image", name: "候选 A", mimeType: "image/png", thumbnailAvailable: true, taskId: "task-1", selected: false }],
  context: { shotId: "shot-1", stage: "IMAGE", snapshotAvailable: true, promptText: "prompt", referenceSets: [], referenceAssets: [] },
  ...overrides,
});

describe("ShotBatchReviewBoard adapter", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("keeps the legacy image/video controls and does not invoke callbacks during render", () => {
    const onSelect = vi.fn();
    const imageShot = { id: "shot-1", ordinal: 0, name: "镜头 1", stageConfigs: [], referenceAssets: [], generationLinks: [{ stage: "image", task: { outputAssetIds: ["asset-a"] } }], status: "READY", imageStatus: "READY", videoStatus: "NOT_STARTED" } as never;
    const videoShot = { id: "shot-2", ordinal: 1, name: "镜头 2", stageConfigs: [], referenceAssets: [], generationLinks: [{ stage: "video", task: { outputAssetIds: ["asset-v"] } }], status: "READY", imageStatus: "READY", videoStatus: "READY" } as never;
    const common = { projectId: "project-1", assets: [asset("asset-a"), asset("asset-v", "video")], busy: false, onAssetsLoaded: vi.fn(), onSelect, onRetry: vi.fn() };
    expect(renderToStaticMarkup(<ShotBatchReviewBoard {...common} shots={[imageShot]} stage="image" />)).toContain("设为关键帧");
    expect(renderToStaticMarkup(<ShotBatchReviewBoard {...common} shots={[videoShot]} stage="video" />)).toContain("设为最终视频");
    expect(renderToStaticMarkup(<ShotBatchReviewBoard {...common} shots={[imageShot]} stage="image" />)).toContain("打开 A/B 对比");
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("builds local A/B items from loaded assets without review status or context", () => {
    const shot = { id: "shot-local", ordinal: 2, name: "本地镜头", stageConfigs: [], referenceAssets: [], generationLinks: [{ stage: "image", task: { outputAssetIds: ["asset-a"] } }] } as never;
    const local = toLocalCompareItem(shot, [asset("asset-a")], "image", "project-1");
    expect(local.candidates).toHaveLength(1);
    expect(local.reviewStatus).toBeUndefined();
    expect(local.context).toBeUndefined();
    expect(local.candidates[0].selected).toBe(false);
  });

  it("maps the enhanced payload without fetching candidate metadata and separates status from Shot selection", () => {
    const item = reviewItem({ selectedAssetId: "asset-a", reviewStatus: "APPROVED" });
    const mapped = toCompareItem(item, "project-1", { "asset-a": "blob:asset-a" });
    expect(mapped.candidates[0]).toMatchObject({ id: "asset-a", imageUrl: "blob:asset-a", selected: false });
    expect(mapped.selectedCandidateId).toBe("asset-a");
    expect(mapped.reviewStatus).toBe("APPROVED");
    expect(mapped.contextSnapshot?.contextHash).toBeUndefined();
  });

  it("keeps compact filter counts bounded to the already-loaded review items", () => {
    const items = [reviewItem(), reviewItem({ itemId: "item-2", reviewStatus: "APPROVED" }), reviewItem({ itemId: "item-3", reviewStatus: "REGENERATE", taskStatus: "FAILED", productionItemStatus: "FAILED" })];
    expect(reviewCounts(items)).toEqual({ needsReview: 1, approved: 1, regenerate: 1 });
    expect(items.filter((item) => matchesFilter(item, "FAILED"))).toHaveLength(1);
  });

  it("limits review image reads to the current item instead of the whole batch", () => {
    const current = reviewItem({ itemId: "current", candidateAssets: [
      { assetId: "current-a", assetType: "image", name: "A", mimeType: "image/png", thumbnailAvailable: true, selected: false },
      { assetId: "current-video", assetType: "video", name: "视频", mimeType: "video/mp4", thumbnailAvailable: false, selected: false },
    ] });
    const other = reviewItem({ itemId: "other", candidateAssets: [{ assetId: "other-a", assetType: "image", name: "其他", mimeType: "image/png", thumbnailAvailable: true, selected: false }] });
    expect(reviewImageIdsForItem(current)).toEqual(["current-a"]);
    expect(reviewImageIdsForItem(other)).toEqual(["other-a"]);
    expect(reviewImageIdsForItem(undefined)).toEqual([]);
  });

  it("forces regeneration payload autoStart false while retaining the wire field", async () => {
    vi.mocked(invoke).mockResolvedValue({});
    await regenerateProductionItem({ projectId: "project-1", batchId: "batch-1", itemId: "item-1", useOriginalSeed: false, autoStart: true });
    expect(invoke).toHaveBeenCalledWith("production_item_review_regenerate", { request: expect.objectContaining({ projectId: "project-1", batchId: "batch-1", itemId: "item-1", autoStart: false }) });
  });
});
