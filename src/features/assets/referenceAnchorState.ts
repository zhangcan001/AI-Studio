import type { AssetView } from "../../types/asset";
import type { ReferenceAnchorAssetView, ReferenceAnchorKind, ReferenceAnchorView } from "../../types/referenceAnchor";

export const referenceAnchorKindLabels: Record<ReferenceAnchorKind, string> = {
  CHARACTER: "角色",
  SCENE: "场景",
  PROP: "道具",
  STYLE: "风格",
};

export function isImageAsset(asset: AssetView): boolean {
  return asset.assetType === "image"
    || asset.category === "source_image"
    || asset.category === "generated_image"
    || asset.mimeType.startsWith("image/");
}

export function orderedReferenceAnchorAssets(items: ReferenceAnchorAssetView[]): ReferenceAnchorAssetView[] {
  return [...items].sort((left, right) => left.ordinal - right.ordinal);
}

export function orderedReferenceAnchorAssetIds(items: ReferenceAnchorAssetView[]): string[] {
  return orderedReferenceAnchorAssets(items).map((item) => item.assetId);
}

export function filterReferenceAnchors(
  anchors: ReferenceAnchorView[],
  kind: ReferenceAnchorKind | "ALL",
  keyword: string,
): ReferenceAnchorView[] {
  const normalizedKeyword = keyword.trim().toLocaleLowerCase();
  return anchors.filter((anchor) => {
    if (kind !== "ALL" && anchor.kind !== kind) return false;
    return !normalizedKeyword
      || anchor.name.toLocaleLowerCase().includes(normalizedKeyword)
      || anchor.description.toLocaleLowerCase().includes(normalizedKeyword);
  });
}

export function appendUniqueReferenceAssets(
  current: ReferenceAnchorAssetView[],
  selected: AssetView[],
  max = 20,
): ReferenceAnchorAssetView[] {
  const next = [...current];
  const existing = new Set(next.map((item) => item.assetId));
  for (const asset of selected) {
    if (existing.has(asset.id) || next.length >= max) continue;
    next.push({ assetId: asset.id, ordinal: next.length, asset });
    existing.add(asset.id);
  }
  return next;
}
