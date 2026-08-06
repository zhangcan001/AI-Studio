import { useEffect, useState } from "react";
import { getAssetMediaUrl, listAssetsByTask, readAssetImage } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { TaskView } from "../../types/task";
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
          setError(loadError instanceof Error ? loadError.message : String(loadError));
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
        <span className="section-label">Output</span>
        <p>Your generated media will appear here.</p>
      </section>
    );
  }

  return (
    <section className="output-card">
      <div className="section-heading">
        <div>
          <span className="section-label">Output</span>
          <h2>{images.length ? `${images.length} output${images.length === 1 ? "" : "s"}` : "Loading outputs..."}</h2>
        </div>
      </div>
      {error && <p className="error-message">Unable to load output: {error}</p>}
      <div className="output-grid">
        {images.map(({ asset, url }) => (
          <figure key={asset.id} className="asset-card">
            {asset.assetType === "video" || asset.category === "generated_video" ? (
              <VideoOutput asset={asset} src={url} />
            ) : <img src={url} alt={asset.name} />}
            <figcaption>
              <strong>{asset.assetType === "video" || asset.category === "generated_video" ? formatDuration(asset.durationMs) : `${asset.width ?? "--"} × ${asset.height ?? "--"}`}</strong>
              <span>{formatBytes(asset.fileSize)} · {asset.originalName}</span>
            </figcaption>
          </figure>
        ))}
      </div>
    </section>
  );
}

function formatDuration(value?: number | null): string {
  if (!value || value < 0) return "Duration unavailable";
  const totalSeconds = Math.round(value / 1000);
  return `${Math.floor(totalSeconds / 60)}:${String(totalSeconds % 60).padStart(2, "0")}`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
}
