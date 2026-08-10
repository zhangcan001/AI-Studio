import { useEffect, useRef, useState } from "react";
import { getAssetMediaUrl, readAssetImage, readAssetThumbnail } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import { assetDisplayName, assetTypeLabel, formatDateTime, formatDurationMs, formatFileSize } from "../../i18n/statusLabels";

interface Props {
  projectId: string;
  asset: AssetView;
  onSelect: (asset: AssetView) => void;
  compareMode?: boolean;
  compared?: boolean;
  onToggleCompare?: (asset: AssetView) => void;
  onFavorite?: (asset: AssetView) => void;
  selectionMode?: boolean;
  selected?: boolean;
  onToggleSelection?: (asset: AssetView) => void;
}

export function AssetCard({ projectId, asset, onSelect, compareMode = false, compared = false, onToggleCompare, onFavorite, selectionMode = false, selected = false, onToggleSelection }: Props) {
  const cardRef = useRef<HTMLButtonElement>(null);
  const [visible, setVisible] = useState(false);
  const [previewUrl, setPreviewUrl] = useState<string>();

  useEffect(() => {
    const element = cardRef.current;
    if (!element || typeof IntersectionObserver === "undefined") {
      setVisible(true);
      return () => undefined;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setVisible(true);
          observer.disconnect();
        }
      },
      { rootMargin: "200px" },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!visible) return () => undefined;
    const isVideo = asset.assetType === "video" || asset.category === "generated_video" || asset.category === "source_video";
    const isAudio = asset.assetType === "audio" || asset.category === "source_audio";
    if (isVideo && !asset.thumbnailAvailable || isAudio) {
      setPreviewUrl(undefined);
      return () => undefined;
    }
    let active = true;
    let url: string | undefined;
    const readPreview = asset.thumbnailAvailable
      ? readAssetThumbnail(projectId, asset.id).catch(() => readAssetImage(projectId, asset.id))
      : readAssetImage(projectId, asset.id);
    void readPreview
      .then((bytes) => {
        if (!active) return;
        url = URL.createObjectURL(new Blob([bytes], { type: asset.thumbnailAvailable ? "image/png" : asset.mimeType }));
        setPreviewUrl(url);
      })
      .catch(() => {
        if (active) setPreviewUrl(undefined);
      });
    return () => {
      active = false;
      if (url) URL.revokeObjectURL(url);
    };
  }, [asset.assetType, asset.category, asset.id, asset.mimeType, asset.thumbnailAvailable, projectId, visible]);

  const isVideo = asset.assetType === "video" || asset.category === "generated_video" || asset.category === "source_video";
  const isAudio = asset.assetType === "audio" || asset.category === "source_audio";
  const mediaUrl = isVideo ? getAssetMediaUrl(projectId, asset.id, "video") : isAudio ? getAssetMediaUrl(projectId, asset.id, "audio") : undefined;
  const typeLabel = assetTypeLabel(asset);
  const displayName = assetDisplayName(asset);

  return (
    <article className={`asset-library-card${compareMode && compared ? " asset-library-card-compared" : ""}`}>
      {selectionMode && (
        <label className="asset-bulk-selector">
          <input type="checkbox" checked={selected} onChange={() => onToggleSelection?.(asset)} aria-label={`选择素材${displayName}`} />
          <span>选择</span>
        </label>
      )}
      <button ref={cardRef} type="button" className="asset-library-card-main" aria-pressed={compareMode ? compared : undefined} onClick={() => (compareMode && onToggleCompare ? onToggleCompare(asset) : onSelect(asset))}>
      <span className="asset-library-image">
        {previewUrl ? (
          <img src={previewUrl} alt={displayName} loading="lazy" />
        ) : isVideo && mediaUrl ? (
          <video src={mediaUrl} aria-label={displayName} preload="metadata" muted playsInline />
        ) : (
          <span className="asset-image-placeholder">{isAudio ? "音频素材" : isVideo ? "视频预览" : "暂无预览"}</span>
        )}
        {compareMode && !isAudio && <span className="asset-compare-badge">{compared ? "已选对比" : "加入对比"}</span>}
      </span>
      <span className="asset-library-card-copy">
        <strong>{displayName}</strong>
        <span>{typeLabel}</span>
        <small>
          {isVideo || isAudio ? formatDurationMs(asset.durationMs) : `${asset.width ?? "--"} × ${asset.height ?? "--"}`} · {formatFileSize(asset.fileSize)}
        </small>
        <small>{formatDateTime(asset.createdAt)}</small>
        <span className="asset-tag-chips" aria-label="素材标签">
          {asset.tags.slice(0, 3).map((tag) => <span key={tag.id}>{tag.name}</span>)}
          {asset.tags.length > 3 && <span>+{asset.tags.length - 3}</span>}
        </span>
      </span>
      </button>
      {onFavorite && (
        <button type="button" className="asset-favorite-button" aria-label={asset.isFavorite ? "取消收藏素材" : "收藏素材"} aria-pressed={asset.isFavorite} onClick={() => onFavorite(asset)}>
          <span aria-hidden="true">{asset.isFavorite ? "★" : "☆"}</span>
          {asset.isFavorite ? "已收藏" : "收藏"}
        </button>
      )}
    </article>
  );
}
