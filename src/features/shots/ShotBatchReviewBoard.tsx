import { useEffect, useMemo, useState } from "react";
import { getAsset, getAssetMediaUrl, readAssetImage, readAssetThumbnail } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { ShotStage, ShotView } from "../../types/shot";
import { deriveStageStatus, recentShotFailure, shotStatusLabels } from "./shotDomain";
import { stageLabel } from "./shotBatchDomain";

interface Props {
  projectId: string;
  shots: ShotView[];
  assets: AssetView[];
  stage: ShotStage;
  onAssetsLoaded: (assets: AssetView[]) => void;
  onSelect: (shotId: string, stage: ShotStage, assetId: string, fromLinkedTask: boolean) => void;
  onRetry: (shotId: string, stage: ShotStage) => void;
  onOpenTask?: (taskId: string) => void;
}

export function ShotBatchReviewBoard({ projectId, shots, assets, stage, onAssetsLoaded, onSelect, onRetry, onOpenTask }: Props) {
  const candidateIds = useMemo(() => [...new Set(shots.flatMap((shot) => shot.generationLinks.filter((link) => link.stage === stage).flatMap((link) => link.task?.outputAssetIds ?? [])))] , [shots, stage]);
  useEffect(() => {
    const missing = candidateIds.filter((id) => !assets.some((asset) => asset.id === id));
    if (!missing.length) return;
    let active = true;
    void Promise.all(missing.slice(0, 100).map((id) => getAsset(projectId, id).catch(() => undefined))).then((loaded) => {
      if (active) onAssetsLoaded(loaded.filter((asset): asset is AssetView => Boolean(asset)));
    });
    return () => { active = false; };
  }, [assets, candidateIds, onAssetsLoaded, projectId]);

  return (
    <section className="shot-batch-review-board" aria-label={`${stageLabel(stage)}批量复核`}>
      <div className="shot-block-heading"><div><span className="section-label">人工复核</span><h3>{stage === "image" ? "关键帧候选复核" : "最终视频候选复核"}</h3></div><span className="shot-inline-note">每个 Shot 必须明确选择结果后才进入下一阶段。</span></div>
      <div className="shot-batch-review-grid">
        {shots.map((shot) => {
          const status = deriveStageStatus(shot, stage);
          const links = shot.generationLinks.filter((link) => link.stage === stage);
          const ids = new Set(links.flatMap((link) => link.task?.outputAssetIds ?? []));
          const candidates = assets.filter((asset) => ids.has(asset.id) && (stage === "image" ? asset.assetType === "image" : asset.assetType === "video"));
          const selectedId = stage === "image" ? shot.selectedImageAssetId : shot.selectedVideoAssetId;
          const failure = recentShotFailure(shot, stage);
          return (
            <article key={shot.id} className="shot-batch-review-card">
              <div className="shot-batch-review-card-heading"><div><strong>{String(shot.ordinal + 1).padStart(2, "0")} · {shot.name}</strong><small>{shotStatusLabels[status]}</small></div>{selectedId && <span className="shot-batch-selected-badge">已选择</span>}</div>
              {candidates.length > 0 ? <div className="shot-batch-review-candidates">{candidates.map((asset) => <div key={asset.id} className={`shot-batch-review-candidate${selectedId === asset.id ? " selected" : ""}`}><ReviewAssetMedia projectId={projectId} asset={asset} stage={stage} /><div><strong>{asset.name}</strong><small>{selectedId === asset.id ? "当前选择" : "候选结果"}</small></div><button type="button" onClick={() => onSelect(shot.id, stage, asset.id, true)}>{selectedId === asset.id ? "已选" : stage === "image" ? "设为关键帧" : "设为最终视频"}</button></div>)}</div> : <p className="empty-state">{status === "GENERATING_IMAGE" || status === "GENERATING_VIDEO" ? "任务运行中，等待结果…" : "暂无候选结果"}</p>}
              {failure && <div className="shot-batch-review-failure"><strong>最近一次任务失败</strong><span>{failure.error?.message ?? "生成任务失败，需要处理"}</span><div><button type="button" className="quiet-button" onClick={() => onRetry(shot.id, stage)}>重新加入队列</button>{onOpenTask && <button type="button" className="quiet-button" onClick={() => failure.id && onOpenTask(failure.id)}>查看任务详情</button>}</div></div>}
            </article>
          );
        })}
        {!shots.length && <p className="empty-state">还没有 Shot。</p>}
      </div>
    </section>
  );
}

function ReviewAssetMedia({ projectId, asset, stage }: { projectId: string; asset: AssetView; stage: ShotStage }) {
  const [url, setUrl] = useState<string>();

  useEffect(() => {
    if (stage === "video") return undefined;
    let active = true;
    let objectUrl: string | undefined;
    const load = asset.thumbnailAvailable
      ? readAssetThumbnail(projectId, asset.id).catch(() => readAssetImage(projectId, asset.id))
      : readAssetImage(projectId, asset.id);
    void load.then((bytes) => {
      if (!active) return;
      objectUrl = URL.createObjectURL(new Blob([bytes], { type: asset.mimeType }));
      setUrl(objectUrl);
    }).catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [asset.id, asset.mimeType, asset.thumbnailAvailable, projectId, stage]);

  if (stage === "video") {
    return <video className="shot-batch-review-media" src={getAssetMediaUrl(projectId, asset.id, "video")} controls preload="metadata" playsInline aria-label={asset.name} />;
  }
  return <div className="shot-batch-review-thumb">{url ? <img src={url} alt={asset.name} /> : <span>图片</span>}</div>;
}
