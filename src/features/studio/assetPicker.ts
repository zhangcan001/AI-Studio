import type { AssetView } from "../../types/asset";

export type AssetPickerKind = "image" | "video" | "audio";
export type AssetPickerFilter = "all" | "source" | "generated";

export function isAssetPickerCompatible(asset: AssetView, kind: AssetPickerKind): boolean {
  if (kind === "image") {
    return asset.assetType === "image" || asset.category === "source_image" || asset.category === "generated_image";
  }
  if (kind === "video") {
    return asset.assetType === "video" && ["source_video", "generated_video"].includes(asset.category);
  }
  return asset.assetType === "audio" && asset.category === "source_audio";
}

export function filterPickerAssets(
  assets: AssetView[],
  kind: AssetPickerKind,
  filter: AssetPickerFilter,
): AssetView[] {
  return assets.filter((asset) => {
    if (!isAssetPickerCompatible(asset, kind)) return false;
    if (filter === "source") return asset.category.startsWith("source_");
    if (filter === "generated") return asset.category.startsWith("generated_");
    return true;
  });
}

export function toggleAssetSelection(
  selectedIds: string[],
  assetId: string,
  multiple: boolean,
  maxItems: number,
): string[] {
  if (!multiple) return [assetId];
  if (selectedIds.includes(assetId)) return selectedIds.filter((id) => id !== assetId);
  if (selectedIds.length >= maxItems) return selectedIds;
  return [...selectedIds, assetId];
}

export function applyAssetPickerAction(
  committedIds: string[],
  draftIds: string[],
  action: "cancel" | "confirm",
): string[] {
  return action === "confirm" ? [...draftIds] : [...committedIds];
}
