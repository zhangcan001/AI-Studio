import {
  getAssetMediaUrl,
  type ProductionBatchReviewProductivity,
} from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import type {
  ProductionMonitorBatchReadModel,
  ProductionMonitorItemReadModel,
} from "../production/ProductionMonitor";

function isVideoAsset(asset: Pick<AssetView, "assetType" | "category">): boolean {
  return asset.assetType === "video" || asset.category === "source_video" || asset.category === "generated_video";
}

export function monitorReadModelFor(
  batch: ProductionBatchDetail | undefined,
  review: ProductionBatchReviewProductivity | undefined,
  projectId: string,
): ProductionMonitorBatchReadModel | undefined {
  const source = batch ?? review?.batch;
  if (!source) return undefined;
  const reviewItems = new Map((review?.items ?? []).map((item) => [item.itemId, item]));
  const sourceItems = source.items.length
    ? source.items
    : (review?.items ?? []).map((item) => ({
      id: item.itemId,
      ordinal: item.ordinal,
      workflowVersionId: item.workflowVersionId,
      recipeId: item.recipeId,
      status: item.productionItemStatus as ProductionBatchDetail["items"][number]["status"],
      taskId: item.taskId,
      errorCode: undefined,
      errorMessage: undefined,
    }));
  const items: ProductionMonitorItemReadModel[] = sourceItems.map((item) => {
    const reviewItem = reviewItems.get(item.id);
    const outputAssets = reviewItem?.outputAssets ?? [];
    const candidates = reviewItem?.candidateAssets ?? [];
    const explicitlySelectedAsset = outputAssets.find((asset) => asset.id === reviewItem?.selectedAssetId);
    const selectedAsset = (explicitlySelectedAsset && isVideoAsset(explicitlySelectedAsset))
      ? explicitlySelectedAsset
      : outputAssets.find(isVideoAsset) ?? explicitlySelectedAsset ?? outputAssets[0];
    const candidateOutputs = candidates.map((candidate) => ({
      assetId: candidate.assetId,
      assetType: candidate.assetType,
      name: candidate.name,
      mimeType: candidate.mimeType,
      localPath: candidate.localPath,
      width: candidate.width,
      height: candidate.height,
      thumbnailAvailable: candidate.thumbnailAvailable,
      selected: candidate.selected,
      reviewResult: candidate.reviewResult,
      asset: outputAssets.find((asset) => asset.id === candidate.assetId),
    }));
    const candidateWithLocation = candidates.find((candidate) => candidate.assetId === selectedAsset?.id && candidate.localPath);
    const assetOutput = selectedAsset ? {
      ...selectedAsset,
      candidates: candidateOutputs,
    } : candidates.length ? { candidates: candidateOutputs } : undefined;
    return {
      id: item.id,
      ordinal: item.ordinal,
      status: item.status,
      name: reviewItem?.shotId ?? reviewItem?.promptText ?? item.id,
      promptText: reviewItem?.promptText ?? ("promptText" in item ? item.promptText : undefined),
      errorCode: item.errorCode,
      errorMessage: item.errorMessage,
      assetId: selectedAsset?.id ?? candidates[0]?.assetId,
      videoUrl: selectedAsset && (selectedAsset.assetType === "video" || selectedAsset.category === "generated_video")
        ? getAssetMediaUrl(projectId, selectedAsset.id, "video")
        : undefined,
      localPath: candidateWithLocation?.localPath,
      output: assetOutput,
      media: assetOutput,
      recordAvailable: Boolean(selectedAsset || candidates.length),
    };
  });
  return {
    id: source.id,
    name: source.name,
    status: source.status,
    total: source.total,
    pending: source.pending,
    running: source.running,
    succeeded: source.succeeded,
    failed: source.failed,
    cancelled: source.cancelled,
    skipped: source.skipped,
    items,
  };
}

type MonitorReviewItem = ProductionBatchReviewProductivity["items"][number];

export function isVideoCandidate(candidate: { assetType?: string; mimeType?: string }): boolean {
  return `${candidate.assetType ?? ""} ${candidate.mimeType ?? ""}`.toLowerCase().includes("video");
}

export function monitorCandidateFor(item: MonitorReviewItem, videoOnly = false) {
  const candidates = item.candidateAssets;
  return candidates.find((candidate) => candidate.assetId === item.selectedAssetId && (!videoOnly || isVideoCandidate(candidate)))
    ?? candidates.find((candidate) => !videoOnly || isVideoCandidate(candidate));
}

export function firstFinishedMonitorAsset(review?: ProductionBatchReviewProductivity): { itemId: string; assetId: string; localPath?: string } | undefined {
  for (const item of review?.items ?? []) {
    if (item.productionItemStatus !== "SUCCEEDED") continue;
    const candidate = monitorCandidateFor(item, true);
    if (candidate) return { itemId: item.itemId, assetId: candidate.assetId, localPath: candidate.localPath };
  }
  return undefined;
}

export function safeManifestPart(value: string): string {
  return value.replace(/[^a-zA-Z0-9._-]+/g, "_").replace(/^\.+|\.+$/g, "") || "batch";
}

export function buildLocalDeliveryManifest(
  batch: ProductionMonitorBatchReadModel,
  review: ProductionBatchReviewProductivity,
  expectedBatchId: string,
) {
  const batchId = String(batch.id ?? batch.batchId ?? "");
  const reviewBatchId = String(review.batch.id ?? "");
  const batchItems = batch.items ?? [];
  const reviewItems = review.items ?? [];
  const batchItemIds = new Set(batchItems.map((item) => String(item.id ?? item.itemId ?? "")));
  const reviewItemIds = new Set(reviewItems.map((item) => item.itemId));
  const itemsMatch = batchItems.length === reviewItems.length
    && batchItemIds.size === batchItems.length
    && reviewItemIds.size === reviewItems.length
    && [...batchItemIds].every((itemId) => reviewItemIds.has(itemId));

  if (
    !expectedBatchId
    || batchId !== expectedBatchId
    || reviewBatchId !== expectedBatchId
    || (batch.total !== undefined && review.total !== undefined && batch.total !== review.total)
    || !itemsMatch
  ) {
    return undefined;
  }

  const items = [...reviewItems]
    .sort((left, right) => left.ordinal - right.ordinal || left.itemId.localeCompare(right.itemId))
    .map((item) => {
      const candidate = monitorCandidateFor(item, true);
      return {
        externalId: item.shotId ?? item.itemId,
        itemId: item.itemId,
        status: item.productionItemStatus,
        videoAssetId: candidate?.assetId,
        videoPath: candidate?.localPath,
      };
    });

  return {
    manifestType: "LOCAL_DELIVERY_MANIFEST" as const,
    manifestVersion: 1 as const,
    batchId,
    batchName: batch.name ?? batch.batchName ?? review.batch.name,
    generatedAt: new Date().toISOString(),
    total: batch.total ?? review.total ?? items.length,
    succeeded: batch.succeeded ?? review.successCount,
    failed: batch.failed ?? review.failedCount,
    items,
  };
}
