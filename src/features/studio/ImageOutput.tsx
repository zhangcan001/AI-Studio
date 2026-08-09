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

interface Props {
  projectId: string;
  task?: TaskView;
  onOpenTask?: () => void;
}

export function ImageOutput({ projectId, task, onOpenTask }: Props) {
  const [images, setImages] = useState<LoadedAsset[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const urls: string[] = [];
    setImages([]);
    setSelectedIndex(0);
    setError(null);

    if (!task || task.status !== "SUCCEEDED") return () => undefined;

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
        if (cancelled) loaded.forEach(({ url }) => URL.revokeObjectURL(url));
        else setImages(loaded);
      })
      .catch((loadError: unknown) => {
        if (!cancelled) setError(toUserMessage(loadError));
      });

    return () => {
      cancelled = true;
      urls.forEach((url) => URL.revokeObjectURL(url));
    };
  }, [projectId, task]);

  if (!task) {
    return (
      <section className="output-card empty-output creation-result-panel">
        <span className="section-label">生成结果</span>
        <h2>开始创作</h2>
        <p>生成完成后，图片或视频会显示在这里。</p>
      </section>
    );
  }

  if (task.status !== "SUCCEEDED") {
    return (
      <section className="output-card empty-output creation-result-panel">
        <span className="section-label">生成结果</span>
        <h2>{task.status === "COLLECTING" ? "正在整理生成结果..." : "结果预览"}</h2>
        <p>{task.status === "COLLECTING" ? "结果即将显示在这里。" : "任务完成后，图片或视频会显示在这里。"}</p>
      </section>
    );
  }

  const selected = images[selectedIndex] ?? images[0];
  return (
    <section className="output-card creation-result-panel">
      <div className="section-heading result-heading">
        <div>
          <span className="section-label">生成结果</span>
          <h2>{images.length ? "主预览" : "正在加载结果..."}</h2>
        </div>
        {onOpenTask && <button type="button" className="quiet-button" onClick={onOpenTask}>查看任务详情</button>}
      </div>
      {error && <p className="error-message" role="alert">生成结果加载失败：{error}</p>}
      {selected ? (
        <>
          <div className="result-main-preview">
            {selected.asset.assetType === "video" || selected.asset.category === "generated_video" ? (
              <VideoOutput asset={{ ...selected.asset, name: assetDisplayName(selected.asset) }} src={selected.url} />
            ) : (
              <img src={selected.url} alt={assetDisplayName(selected.asset)} />
            )}
          </div>
          <div className="result-main-caption">
            <strong>{assetDisplayName(selected.asset)}</strong>
            <span>{assetTypeLabel(selected.asset)} · {selected.asset.assetType === "video" || selected.asset.category === "generated_video" ? formatDurationMs(selected.asset.durationMs) : `${selected.asset.width ?? "--"} × ${selected.asset.height ?? "--"}`} · {formatFileSize(selected.asset.fileSize)}</span>
          </div>
          {images.length > 1 && (
            <div className="result-thumbnail-list" aria-label="其他生成结果">
              {images.map(({ asset }, index) => (
                <button key={asset.id} type="button" className={index === selectedIndex ? "result-thumbnail result-thumbnail-active" : "result-thumbnail"} aria-label={`查看第 ${index + 1} 个结果`} aria-pressed={index === selectedIndex} onClick={() => setSelectedIndex(index)}>
                  <span>{index + 1}</span>
                  <small>{assetDisplayName(asset)}</small>
                </button>
              ))}
            </div>
          )}
        </>
      ) : (
        <p className="empty-state">正在读取生成结果...</p>
      )}
    </section>
  );
}
