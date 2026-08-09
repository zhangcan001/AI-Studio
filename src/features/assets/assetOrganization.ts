import type { AssetView } from "../../types/asset";

export function replaceAssetOrganization(items: AssetView[], refreshed: AssetView): AssetView[] {
  return items.map((asset) => asset.id === refreshed.id ? refreshed : asset);
}
