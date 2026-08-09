import type { AssetView } from "../../types/asset";
import { AssetCard } from "./AssetCard";

interface Props {
  projectId: string;
  assets: AssetView[];
  onSelect: (asset: AssetView) => void;
}

export function AssetGrid({ projectId, assets, onSelect }: Props) {
  if (!assets.length) {
    return <p className="empty-state">当前筛选条件下没有找到资产。</p>;
  }
  return (
    <div className="asset-library-grid">
      {assets.map((asset) => (
        <AssetCard key={asset.id} projectId={projectId} asset={asset} onSelect={onSelect} />
      ))}
    </div>
  );
}
