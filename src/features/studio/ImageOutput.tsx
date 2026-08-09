import { useEffect, useState } from "react";
import { getAssetMediaUrl, listAssetsByTask, readAssetImage } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { TaskView } from "../../types/task";
import { toUserMessage } from "../../i18n/errorMessages";
import { assetDisplayName, assetTypeLabel, formatDurationMs, formatFileSize } from "../../i18n/statusLabels";
import { VideoOutput } from "./VideoOutput";

interface LoadedAsset {
  asset: AssetView;
  url: string;
}

export function ImageOutput({ projectId, task }: { projectId: string; task?: TaskView }) {
  const [images, setImages] = useState<LoadedAsset[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const urls: string[] = [];
    setImages([]);
    setError(null);

    if (!task || task.status !== "SUCCEEDED") {
      return () => undefined;
    }

    void listAssetsByTask(projectId, task.id)
      .then(async (assets) => {
        const loaded = await Promise.all(
          assets.map(async (asset) => {
            const isVideo = asset.assetType === "video" || asset.category === "generated_video";
            if (isVideo) return { asset, url: getAssetMediaUrl(projectId, asset.id) };
            const bytes = await readAssetImage(projectId, asset.id);
            const url = URL.createObjectURL(new Blob([bytes], { type: asset.mimeType }));
            urls.push(url);
            return { asset, url };
          }),
        );
        if (cancelled) {
          loaded.forEach(({ url }) => URL.revokeObjectURL(url));
        } else {
          setImages(loaded);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          setError(toUserMessage(loadError));
        }
      });

    return () => {
      cancelled = true;
      urls.forEach((url) => URL.revokeObjectURL(url));
    };
  }, [projectId, task]);

  if (!task || task.status !== "SUCCEEDED") {
    return (
      <section className="output-card empty-output">
        <span className="section-label">输出结果</span>
        <p>生成的媒体会显示在这里。</p>
      </section>
    );
  }

  return (
    <section className="output-card">
      <div className="section-heading">
        <div>
          <span className="section-label">输出结果</span>
          <h2>{images.length ? `${images.length} 个输出结果` : "正在加载输出结果..."}</h2>
        </div>
      </div>
      {error && <p className="error-message">输出结果加载失败：{error}</p>}
      <div className="output-grid">
        {images.map(({ asset, url }) => {
          const displayName = assetDisplayName(asset);
          const displayOriginalName = assetDisplayName(asset, asset.originalName);
          return (
            <figure key={asset.id} className="asset-card">
              {asset.assetType === "video" || asset.category === "generated_video" ? (
                <VideoOutput asset={{ ...asset, name: displayName }} src={url} />
              ) : <img src={url} alt={displayName} />}
              <figcaption>
                <strong>{asset.assetType === "video" || asset.category === "generated_video" ? formatDurationMs(asset.durationMs) : `${asset.width ?? "--"} × ${asset.height ?? "--"}`}</strong>
                <span>{assetTypeLabel(asset)} · {formatFileSize(asset.fileSize)} · {displayOriginalName}</span>
              </figcaption>
            </figure>
          );
        })}
      </div>
    </section>
  );
}
