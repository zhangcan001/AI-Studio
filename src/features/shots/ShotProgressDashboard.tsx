import type { ShotView } from "../../types/shot";
import { deriveStageStatus } from "./shotDomain";
import { shotProgressSummary } from "./shotBatchDomain";

export function shotImagesReady(shots: ShotView[]): number {
  return shots.filter((shot) => ["IMAGE_REVIEW", "IMAGE_SELECTED"].includes(deriveStageStatus(shot, "image"))).length;
}

export function ShotProgressDashboard({ shots }: { shots: ShotView[] }) {
  const summary = shotProgressSummary(shots);
  const imagesReady = shotImagesReady(shots);
  const metrics = [
    ["总镜头", summary.total, "neutral"],
    ["图片已就绪", imagesReady, "muted"],
    ["图片已选", summary.keyframesSelected, "success"],
    ["视频生成中", summary.videoGenerating, "active"],
    ["视频已就绪", summary.pendingVideoReview, "review"],
    ["已完成", summary.completed, "success"],
    ["失败", summary.needsAttention, "danger"],
  ] as const;
  return (
    <section className="shot-progress-dashboard" aria-label="镜头生产进度">
      <div className="shot-progress-heading"><div><span className="section-label">生产总览</span><h3>生产进度</h3></div><span className="shot-progress-count">{summary.completed} / {summary.total} 已完成</span></div>
      <div className="shot-progress-metrics">
        {metrics.map(([label, value, tone]) => <div key={label} className={`shot-progress-metric shot-progress-metric-${tone}`}><span>{label}</span><strong>{value}</strong></div>)}
      </div>
    </section>
  );
}
