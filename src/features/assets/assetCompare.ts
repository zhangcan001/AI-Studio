import type { AssetView } from "../../types/asset";

export type ComparableAssetType = "image" | "video" | "audio";

export function comparableAssetType(asset: AssetView): ComparableAssetType | undefined {
  if (asset.assetType === "image" || asset.assetType === "video" || asset.assetType === "audio") {
    return asset.assetType;
  }
  if (asset.category.endsWith("_image")) return "image";
  if (asset.category.endsWith("_video")) return "video";
  if (asset.category.endsWith("_audio")) return "audio";
  return undefined;
}

export interface CompareSelectionResult {
  assets: AssetView[];
  notice?: string;
}

export function toggleCompareSelection(current: AssetView[], asset: AssetView): CompareSelectionResult {
  const assetType = comparableAssetType(asset);
  if (assetType === "audio") return { assets: current, notice: "音频暂不支持视觉对比。" };

  const existing = current.some((item) => item.id === asset.id);
  if (existing) return { assets: current.filter((item) => item.id !== asset.id) };
  if (current.length >= 4) return { assets: current, notice: "最多同时对比4个素材。" };

  const firstType = current[0] ? comparableAssetType(current[0]) : undefined;
  if (firstType && assetType && firstType !== assetType) {
    return { assets: current, notice: "请选择相同类型的素材进行对比。" };
  }
  return { assets: [...current, asset] };
}
