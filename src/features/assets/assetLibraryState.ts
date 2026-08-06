import type { AssetView } from "../../types/asset";

export function mergeAssetPage(current: AssetView[], incoming: AssetView[], reset: boolean): AssetView[] {
  if (reset) return incoming;
  const byId = new Map(current.map((asset) => [asset.id, asset]));
  incoming.forEach((asset) => byId.set(asset.id, asset));
  return [...byId.values()];
}
