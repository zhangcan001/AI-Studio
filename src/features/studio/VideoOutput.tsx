import type { AssetView } from "../../types/asset";
import { assetDisplayName } from "../../i18n/statusLabels";

export function VideoOutput({ asset, src }: { asset: AssetView; src: string }) {
  return (
    <video
      src={src}
      controls
      preload="metadata"
      playsInline
      aria-label={assetDisplayName(asset)}
    />
  );
}
