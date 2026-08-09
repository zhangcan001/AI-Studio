import type { AssetView } from "../../types/asset";
import { AssetCard } from "./AssetCard";

interface Props {
  projectId: string;
  assets: AssetView[];
  onSelect: (asset: AssetView) => void;
  emptyMessage?: string;
  compareMode?: boolean;
  compareIds?: string[];
  onToggleCompare?: (asset: AssetView) => void;
}

export function AssetGrid({ projectId, assets, onSelect, emptyMessage = "没有找到符合条件的素材。", compareMode, compareIds = [], onToggleCompare }: Props) {
  if (!assets.length) {
    return <p className="empty-state">{emptyMessage}</p>;
  }
  return (
    <div className="asset-library-grid">
      {assets.map((asset) => (
        <AssetCard
          key={asset.id}
          projectId={projectId}
          asset={asset}
          onSelect={onSelect}
          compareMode={compareMode}
          compared={compareIds.includes(asset.id)}
          onToggleCompare={onToggleCompare}
        />
      ))}
    </div>
  );
}
