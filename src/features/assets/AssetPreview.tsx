import { useEffect, useState } from "react";
import { getAssetMediaUrl, readAssetImage, readAssetThumbnail } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import { assetDisplayName, assetTypeLabel, formatDurationMs } from "../../i18n/statusLabels";

interface Props {
  projectId: string;
  asset: AssetView;
  onClose: () => void;
}

export function AssetPreview({ projectId, asset, onClose }: Props) {
  const [url, setUrl] = useState<string>();
  const [posterUrl, setPosterUrl] = useState<string>();
  const [error, setError] = useState<string>();

  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    let posterObjectUrl: string | undefined;
    setUrl(undefined);
    setPosterUrl(undefined);
    setError(undefined);
    const isVideo = asset.assetType === "video" || asset.category === "generated_video" || asset.category === "source_video";
    const isAudio = asset.assetType === "audio" || asset.category === "source_audio";
    if (isVideo || isAudio) {
      setUrl(getAssetMediaUrl(projectId, asset.id, isVideo ? "video" : "audio"));
      setError(undefined);
      if (asset.thumbnailAvailable) {
        void readAssetThumbnail(projectId, asset.id)
          .then((bytes) => {
            if (!active) return;
            posterObjectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
            setPosterUrl(posterObjectUrl);
          })
          .catch(() => undefined);
      }
      return () => {
        active = false;
        if (posterObjectUrl) URL.revokeObjectURL(posterObjectUrl);
      };
    }
    void readAssetImage(projectId, asset.id)
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: asset.mimeType }));
        setUrl(objectUrl);
      })
      .catch(() => {
        if (active) setError("暂无预览，请稍后重试。");
      });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      if (posterObjectUrl) URL.revokeObjectURL(posterObjectUrl);
    };
  }, [asset.assetType, asset.category, asset.id, asset.mimeType, asset.thumbnailAvailable, projectId]);

  const isVideo = asset.assetType === "video" || asset.category === "generated_video" || asset.category === "source_video";
  const isAudio = asset.assetType === "audio" || asset.category === "source_audio";
  const displayName = assetDisplayName(asset);
  const displayOriginalName = assetDisplayName(asset, asset.originalName);

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  return (
    <div className="asset-preview-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="asset-preview-panel"
        role="dialog"
        aria-modal="true"
        aria-label={`${displayName} 预览`}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="section-heading">
          <div>
            <span className="section-label">资产预览</span>
            <h2>{displayName}</h2>
          </div>
          <button type="button" className="quiet-button" onClick={onClose} aria-label="关闭预览">
            关闭
          </button>
        </div>
        <div className="asset-preview-image">
          {isVideo && url ? (
            <video src={url} poster={posterUrl} controls preload="metadata" playsInline aria-label={displayName} />
          ) : isAudio && url ? (
            <audio src={url} controls preload="metadata" aria-label={displayName} />
          ) : url ? <img src={url} alt={displayName} /> : <p>{error ?? "正在加载预览..."}</p>}
        </div>
        <p className="asset-preview-meta">
          {assetTypeLabel(asset)} · {displayOriginalName} · {isVideo || isAudio ? formatDurationMs(asset.durationMs) : `${asset.width ?? "--"} × ${asset.height ?? "--"}`} · {asset.mimeType}
        </p>
      </section>
    </div>
  );
}
