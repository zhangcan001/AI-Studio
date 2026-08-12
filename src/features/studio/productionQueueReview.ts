import type { AssetView } from "../../types/asset";

export function isReviewableVideoAsset(asset: Pick<AssetView, "assetType" | "category" | "mimeType">): boolean {
  return asset.assetType === "video"
    || asset.category === "generated_video"
    || asset.mimeType.toLowerCase().startsWith("video/");
}

export function hasReviewableVideoOutput(outputAssetsByItem: Record<string, AssetView[]>): boolean {
  return Object.values(outputAssetsByItem).some((assets) => assets.some(isReviewableVideoAsset));
}
