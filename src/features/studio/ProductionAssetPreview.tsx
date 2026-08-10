import { useEffect, useState } from "react";
import { getAssetMediaUrl, readAssetImage } from "../../services/tauriClient";
import { assetDisplayName, assetTypeLabel, formatFileSize } from "../../i18n/statusLabels";
import type { AssetView } from "../../types/asset";

interface Props {
  projectId: string;
  asset: AssetView;
  onClose: () => void;
  onOpenTask?: (taskId: string) => void;
}

export function ProductionAssetPreview({ projectId, asset, onClose, onOpenTask }: Props) {
  const [imageUrl, setImageUrl] = useState<string>();
  const [error, setError] = useState<string>();
  const isVideo = asset.assetType === "video" || asset.category === "generated_video" || asset.category === "source_video";
  const displayName = assetDisplayName(asset);

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  useEffect(() => {
    if (isVideo) return () => undefined;
    let active = true;
    let objectUrl: string | undefined;
    setImageUrl(undefined);
    setError(undefined);
    void readAssetImage(projectId, asset.id)
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: asset.mimeType }));
        setImageUrl(objectUrl);
      })
      .catch(() => {
        if (active) setError("原图读取失败，请稍后重试。");
      });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [asset.id, asset.mimeType, isVideo, projectId]);

  const mediaUrl = isVideo ? getAssetMediaUrl(projectId, asset.id, "video") : undefined;

  return (
    <div className="production-asset-preview-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="production-asset-preview-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`${displayName} 全图预览`}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="production-asset-preview-heading">
          <div>
            <span className="section-label">全图预览</span>
            <strong>{displayName}</strong>
          </div>
          <div className="production-asset-preview-actions">
            {asset.sourceTaskId && onOpenTask && (
              <button type="button" className="quiet-button" onClick={() => { onClose(); onOpenTask(asset.sourceTaskId!); }}>
                查看任务
              </button>
            )}
            <button type="button" className="quiet-button" onClick={onClose} aria-label="关闭全图预览">
              关闭
            </button>
          </div>
        </div>
        <div className="production-asset-preview-media">
          {isVideo && mediaUrl ? (
            <video src={mediaUrl} controls preload="metadata" playsInline aria-label={displayName} />
          ) : imageUrl ? (
            <img src={imageUrl} alt={displayName} />
          ) : (
            <p>{error ?? "正在加载原图..."}</p>
          )}
        </div>
        <p className="production-asset-preview-meta">
          {assetTypeLabel(asset)} · {asset.width ?? "--"} × {asset.height ?? "--"} · {formatFileSize(asset.fileSize)}
        </p>
      </section>
    </div>
  );
}
