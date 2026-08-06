import { useEffect, useState } from "react";
import { getAssetMediaUrl, readAssetImage, readAssetThumbnail } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";

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
    const isVideo = asset.assetType === "video" || asset.category === "generated_video";
    if (isVideo) {
      setUrl(getAssetMediaUrl(projectId, asset.id));
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
        if (active) setError("Preview unavailable");
      });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      if (posterObjectUrl) URL.revokeObjectURL(posterObjectUrl);
    };
  }, [asset.assetType, asset.category, asset.id, asset.mimeType, asset.thumbnailAvailable, projectId]);

  const isVideo = asset.assetType === "video" || asset.category === "generated_video";

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
        aria-label={`${asset.name} preview`}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="section-heading">
          <div>
            <span className="section-label">Asset preview</span>
            <h2>{asset.name}</h2>
          </div>
          <button type="button" className="quiet-button" onClick={onClose} aria-label="Close preview">
            Close
          </button>
        </div>
        <div className="asset-preview-image">
          {isVideo && url ? (
            <video src={url} poster={posterUrl} controls preload="metadata" playsInline aria-label={asset.name} />
          ) : url ? <img src={url} alt={asset.name} /> : <p>{error ?? "Loading preview..."}</p>}
        </div>
        <p className="asset-preview-meta">
          {asset.originalName} · {isVideo ? formatDuration(asset.durationMs) : `${asset.width ?? "--"} × ${asset.height ?? "--"}`} · {asset.mimeType}
        </p>
      </section>
    </div>
  );
}

function formatDuration(value?: number | null): string {
  if (!value || value < 0) return "Duration unavailable";
  const totalSeconds = Math.round(value / 1000);
  return `${Math.floor(totalSeconds / 60)}:${String(totalSeconds % 60).padStart(2, "0")}`;
}
