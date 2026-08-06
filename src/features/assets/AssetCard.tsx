import { useEffect, useRef, useState } from "react";
import { readAssetImage } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";

interface Props {
  projectId: string;
  asset: AssetView;
  onSelect: (asset: AssetView) => void;
}

export function AssetCard({ projectId, asset, onSelect }: Props) {
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
    let active = true;
    let url: string | undefined;
    void readAssetImage(projectId, asset.id)
      .then((bytes) => {
        if (!active) return;
        url = URL.createObjectURL(new Blob([bytes], { type: asset.mimeType }));
        setPreviewUrl(url);
      })
      .catch(() => {
        if (active) setPreviewUrl(undefined);
      });
    return () => {
      active = false;
      if (url) URL.revokeObjectURL(url);
    };
  }, [asset.id, asset.mimeType, projectId, visible]);

  return (
    <button ref={cardRef} type="button" className="asset-library-card" onClick={() => onSelect(asset)}>
      <span className="asset-library-image">
        {previewUrl ? (
          <img src={previewUrl} alt={asset.name} loading="lazy" />
        ) : (
          <span className="asset-image-placeholder">Preview unavailable</span>
        )}
      </span>
      <span className="asset-library-card-copy">
        <strong>{asset.name}</strong>
        <span>{asset.category === "source_image" ? "Source image" : "Generated image"}</span>
        <small>
          {asset.width} × {asset.height} · {formatBytes(asset.fileSize)}
        </small>
        <small>{formatDate(asset.createdAt)}</small>
      </span>
    </button>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
}

function formatDate(value: string): string {
  return new Date(value).toLocaleString();
}
