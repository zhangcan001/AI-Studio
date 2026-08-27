import { getAssetMediaUrl } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { ReviewCompareMediaKind } from "../../types/reviewProductivity";
import { ZoomableImagePreview } from "../shots/ZoomableImagePreview";

export interface ReviewCompareMediaProps {
  asset: AssetView;
  mediaKind?: ReviewCompareMediaKind;
  /** Required for images. The host may provide an object URL or another safe URL. */
  imageUrl?: string;
  /** Optional pre-resolved URL. No asset bytes are loaded by this component. */
  mediaUrl?: string;
  projectId?: string;
  label?: string;
  className?: string;
}

function mediaKindFor(asset: AssetView, mediaKind?: ReviewCompareMediaKind): ReviewCompareMediaKind {
  if (mediaKind) return mediaKind;
  return asset.assetType === "video" || asset.category === "generated_video" ? "video" : "image";
}

export function ReviewCompareMedia({
  asset,
  mediaKind,
  imageUrl,
  mediaUrl,
  projectId,
  label = asset.name,
  className,
}: ReviewCompareMediaProps) {
  const kind = mediaKindFor(asset, mediaKind);
  if (kind === "image") {
    const resolvedImageUrl = imageUrl ?? mediaUrl;
    return resolvedImageUrl ? (
      <ZoomableImagePreview
        imageUrl={resolvedImageUrl}
        alt={label}
        label={`${label}图片预览`}
        className={className ?? "review-compare-image-preview"}
        resetKey={asset.id}
      />
    ) : (
      <div className={`review-compare-media-empty${className ? ` ${className}` : ""}`} role="img" aria-label={`${label}暂无图片 URL`}>
        <span>暂无图片预览</span>
      </div>
    );
  }

  const resolvedMediaUrl = mediaUrl ?? (projectId ? getAssetMediaUrl(projectId, asset.id, "video") : undefined);
  return resolvedMediaUrl ? (
    <video
      className={`review-compare-video${className ? ` ${className}` : ""}`}
      src={resolvedMediaUrl}
      controls
      preload="metadata"
      playsInline
      aria-label={label}
    />
  ) : (
    <div className={`review-compare-media-empty${className ? ` ${className}` : ""}`} role="img" aria-label={`${label}暂无视频 URL`}>
      <span>暂无视频预览</span>
    </div>
  );
}
