import { useEffect, useState } from "react";
import { readAssetImage } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";

interface Props {
  asset: AssetView;
  onClose: () => void;
}

export function AssetPreview({ asset, onClose }: Props) {
  const [url, setUrl] = useState<string>();
  const [error, setError] = useState<string>();

  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    void readAssetImage(asset.id)
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
    };
  }, [asset.id, asset.mimeType]);

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
          {url ? <img src={url} alt={asset.name} /> : <p>{error ?? "Loading preview..."}</p>}
        </div>
        <p className="asset-preview-meta">
          {asset.originalName} · {asset.width} × {asset.height} · {asset.mimeType}
        </p>
      </section>
    </div>
  );
}
