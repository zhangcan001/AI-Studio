import type { AssetView } from "../../types/asset";
import { AssetCard } from "./AssetCard";

interface Props {
  assets: AssetView[];
  onSelect: (asset: AssetView) => void;
}

export function AssetGrid({ assets, onSelect }: Props) {
  if (!assets.length) {
    return <p className="empty-state">No assets found in this category.</p>;
  }
  return (
    <div className="asset-library-grid">
      {assets.map((asset) => (
        <AssetCard key={asset.id} asset={asset} onSelect={onSelect} />
      ))}
    </div>
  );
}
