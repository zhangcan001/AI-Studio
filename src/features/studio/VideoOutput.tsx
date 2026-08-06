import type { AssetView } from "../../types/asset";

export function VideoOutput({ asset, src }: { asset: AssetView; src: string }) {
  return (
    <video
      src={src}
      controls
      preload="metadata"
      playsInline
      aria-label={asset.name}
    />
  );
}
