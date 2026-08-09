import { useEffect, useRef, useState } from "react";
import { getAssetMediaUrl, readAssetImage, readAssetThumbnail } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import { assetDisplayName, assetTypeLabel, formatDateTime, formatDurationMs, formatFileSize } from "../../i18n/statusLabels";

interface Props {
  projectId: string;
  assets: AssetView[];
  onRemove: (assetId: string) => void;
  onClear: () => void;
  onClose: () => void;
}

export function AssetCompareWorkspace({ projectId, assets, onRemove, onClear, onClose }: Props) {
  const videoRefs = useRef<Array<HTMLVideoElement | null>>([]);
  const isVideo = assets[0]?.assetType === "video" || assets[0]?.category.endsWith("_video");

  function playAll() {
    videoRefs.current.forEach((video) => void video?.play().catch(() => undefined));
  }

  function pauseAll() {
    videoRefs.current.forEach((video) => video?.pause());
  }

  function resetAll() {
    videoRefs.current.forEach((video) => {
      if (video) video.currentTime = 0;
    });
  }

  return (
    <div className="asset-compare-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="asset-compare-workspace" role="dialog" aria-modal="true" aria-label="素材对比" onMouseDown={(event) => event.stopPropagation()}>
        <div className="section-heading workspace-heading">
          <div>
            <span className="section-label">对比工作台</span>
            <h2>{isVideo ? "视频对比" : "图片对比"}</h2>
            <p className="section-description">已选择 {assets.length} 个素材。</p>
          </div>
          <div className="asset-compare-actions">
            {isVideo && <>
              <button type="button" onClick={playAll}>全部播放</button>
              <button type="button" className="quiet-button" onClick={pauseAll}>全部暂停</button>
              <button type="button" className="quiet-button" onClick={resetAll}>回到开头</button>
            </>}
            <button type="button" className="quiet-button" onClick={onClear}>清空对比</button>
            <button type="button" className="quiet-button" onClick={onClose}>关闭</button>
          </div>
        </div>
        <div className={`asset-compare-grid asset-compare-grid-${Math.min(assets.length, 4)}`}>
          {assets.map((asset, index) => (
            <CompareAsset
              key={asset.id}
              projectId={projectId}
              asset={asset}
              videoRef={(element) => { videoRefs.current[index] = element; }}
              onRemove={() => onRemove(asset.id)}
            />
          ))}
        </div>
      </section>
    </div>
  );
}

function CompareAsset({
  projectId,
  asset,
  videoRef,
  onRemove,
}: {
  projectId: string;
  asset: AssetView;
  videoRef: (element: HTMLVideoElement | null) => void;
  onRemove: () => void;
}) {
  const [imageUrl, setImageUrl] = useState<string>();
  const [error, setError] = useState<string>();
  const isVideo = asset.assetType === "video" || asset.category.endsWith("_video");

  useEffect(() => {
    if (isVideo) return () => undefined;
    let active = true;
    let objectUrl: string | undefined;
    void (asset.thumbnailAvailable ? readAssetThumbnail(projectId, asset.id).catch(() => readAssetImage(projectId, asset.id)) : readAssetImage(projectId, asset.id))
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: asset.mimeType }));
        setImageUrl(objectUrl);
      })
      .catch(() => {
        if (active) setError("暂无预览");
      });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [asset.id, asset.mimeType, asset.thumbnailAvailable, isVideo, projectId]);

  return (
    <article className="asset-compare-item">
      <div className="asset-compare-media">
        {isVideo ? (
          <video ref={videoRef} src={getAssetMediaUrl(projectId, asset.id, "video")} controls preload="metadata" playsInline />
        ) : imageUrl ? (
          <img src={imageUrl} alt={assetDisplayName(asset)} />
        ) : (
          <span>{error ?? "正在加载预览..."}</span>
        )}
      </div>
      <div className="asset-compare-item-header">
        <strong>{assetDisplayName(asset)}</strong>
        <button type="button" className="quiet-button" onClick={onRemove}>移除</button>
      </div>
      <dl className="asset-compare-facts">
        <div><dt>类型</dt><dd>{assetTypeLabel(asset)}</dd></div>
        <div><dt>{isVideo ? "时长" : "尺寸"}</dt><dd>{isVideo ? formatDurationMs(asset.durationMs) : `${asset.width ?? "--"} × ${asset.height ?? "--"}`}</dd></div>
        <div><dt>文件大小</dt><dd>{formatFileSize(asset.fileSize)}</dd></div>
        <div><dt>创建时间</dt><dd>{formatDateTime(asset.createdAt)}</dd></div>
      </dl>
    </article>
  );
}
