import { describe, expect, it, vi } from "vitest";
import type { ProductionBatchReviewProductivity } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import {
  buildLocalDeliveryManifest,
  firstFinishedMonitorAsset,
  monitorCandidateFor,
  monitorReadModelFor,
} from "./shotProductionMonitorModel";

const mocks = vi.hoisted(() => ({ getAssetMediaUrl: vi.fn() }));
vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, getAssetMediaUrl: mocks.getAssetMediaUrl };
});

const asset = (id: string, assetType: "image" | "video"): AssetView => ({
  id,
  assetType,
  category: assetType === "video" ? "generated_video" : "generated_image",
  name: id,
  originalName: id,
  mimeType: `${assetType}/test`,
  fileSize: 1,
  createdAt: "2026-09-09T00:00:00Z",
  isFavorite: false,
  tags: [],
});

function detail(status: ProductionBatchDetail["status"] = "COMPLETED"): ProductionBatchDetail {
  return {
    id: "batch-a",
    projectId: "project-1",
    name: "Batch A",
    status,
    continueOnFailure: false,
    createdAt: "2026-09-09T00:00:00Z",
    updatedAt: "2026-09-09T00:00:00Z",
    total: 1,
    pending: status === "RUNNING" ? 1 : 0,
    running: status === "RUNNING" ? 1 : 0,
    succeeded: status === "COMPLETED" ? 1 : 0,
    failed: 0,
    cancelled: 0,
    skipped: 0,
    items: [{
      id: "item-a",
      ordinal: 1,
      workflowVersionId: "workflow-a",
      recipeId: "recipe-a",
      status: status === "COMPLETED" ? "SUCCEEDED" : "DISPATCHED",
    }],
  };
}

function review(): ProductionBatchReviewProductivity {
  return {
    batch: detail(),
    total: 1,
    successCount: 1,
    failedCount: 0,
    unreviewedCount: 0,
    approvedCount: 0,
    starredCount: 0,
    regenerateCount: 0,
    rejectedCount: 0,
    items: [{
      itemId: "item-a",
      ordinal: 1,
      taskStatus: "SUCCEEDED",
      productionItemStatus: "SUCCEEDED",
      reviewStatus: "UNREVIEWED",
      reviewNote: "",
      preferred: true,
      workflowVersionId: "workflow-a",
      recipeId: "recipe-a",
      qualityProfile: "QUALITY",
      createdAt: "2026-09-09T00:00:00Z",
      outputAssets: [asset("asset-image", "image"), asset("asset-video", "video")],
      reviewable: true,
      candidateAssets: [{
        assetId: "asset-video",
        assetType: "video",
        name: "asset-video",
        mimeType: "video/mp4",
        localPath: "C:/outputs/asset-video.mp4",
        thumbnailAvailable: true,
        selected: true,
        reviewResult: "APPROVED",
      }],
      context: { snapshotAvailable: true, referenceSets: [], referenceAssets: [] },
    }],
  };
}

describe("shotProductionMonitorModel", () => {
  it("maps monitor data and selects the finished video candidate", () => {
    mocks.getAssetMediaUrl.mockReturnValue("asset://asset-video");
    const nextReview = review();
    const model = monitorReadModelFor(detail(), nextReview, "project-1");

    expect(model?.items?.[0]).toMatchObject({
      id: "item-a",
      assetId: "asset-video",
      videoUrl: "asset://asset-video",
      localPath: "C:/outputs/asset-video.mp4",
      recordAvailable: true,
    });
    expect(firstFinishedMonitorAsset(nextReview)).toEqual({
      itemId: "item-a",
      assetId: "asset-video",
      localPath: "C:/outputs/asset-video.mp4",
    });
    expect(monitorCandidateFor(nextReview.items[0], true)?.assetId).toBe("asset-video");
  });

  it("rejects a delivery manifest when batch identity or item sets disagree", () => {
    const nextReview = review();
    const model = monitorReadModelFor(detail(), nextReview, "project-1")!;
    expect(buildLocalDeliveryManifest(model, nextReview, "batch-a")).toMatchObject({ batchId: "batch-a", total: 1 });
    expect(buildLocalDeliveryManifest(model, nextReview, "batch-b")).toBeUndefined();
  });
});
