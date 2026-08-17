import { useMemo } from "react";
import type { ShotStage, ShotView } from "../../types/shot";
import { deriveShotStatus, deriveStageStatus } from "./shotDomain";
import { ShotProgressDashboard } from "./ShotProgressDashboard";
import {
  ShotBulkConfigPanel,
  type ShotBulkConfigPanelProps,
} from "./ShotBulkConfigPanel";
import "./ProjectProductionPipeline.css";

export interface ProjectPipelineSummary {
  total: number;
  unconfigured: number;
  imageConfigured: number;
  imageReady: number;
  imageGenerating: number;
  imageReview: number;
  imageSelected: number;
  videoConfigured: number;
  videoReady: number;
  videoGenerating: number;
  videoReview: number;
  completed: number;
  failed: number;
}

export const PROJECT_PIPELINE_STAGES = [
  "SHOTS",
  "IMAGE_CONFIGURED",
  "IMAGE_GENERATING",
  "IMAGE_REVIEW",
  "IMAGE_SELECTED",
  "VIDEO_CONFIGURED",
  "VIDEO_GENERATING",
  "VIDEO_REVIEW",
  "COMPLETED",
] as const;

export type ProjectPipelineStage = typeof PROJECT_PIPELINE_STAGES[number];

function hasStageConfig(shot: ShotView, stage: ShotStage): boolean {
  return shot.stageConfigs.some((config) => config.stage === stage);
}

export function deriveProjectPipelineSummary(shots: ShotView[]): ProjectPipelineSummary {
  return shots.reduce<ProjectPipelineSummary>((summary, shot) => {
    const image = deriveStageStatus(shot, "image");
    const video = deriveStageStatus(shot, "video");
    const imageConfigured = hasStageConfig(shot, "image");
    const videoConfigured = hasStageConfig(shot, "video");

    summary.total += 1;
    summary.imageConfigured += Number(imageConfigured);
    summary.videoConfigured += Number(videoConfigured);
    summary.unconfigured += Number(!imageConfigured || !videoConfigured);
    summary.imageReady += Number(image === "READY");
    summary.imageGenerating += Number(image === "GENERATING_IMAGE");
    summary.imageReview += Number(image === "IMAGE_REVIEW");
    summary.imageSelected += Number(image === "IMAGE_SELECTED");
    summary.videoReady += Number(video === "READY");
    summary.videoGenerating += Number(video === "GENERATING_VIDEO");
    summary.videoReview += Number(video === "VIDEO_REVIEW");
    summary.completed += Number(deriveShotStatus(shot) === "COMPLETED");
    summary.failed += Number(image === "FAILED" || video === "FAILED");
    return summary;
  }, {
    total: 0,
    unconfigured: 0,
    imageConfigured: 0,
    imageReady: 0,
    imageGenerating: 0,
    imageReview: 0,
    imageSelected: 0,
    videoConfigured: 0,
    videoReady: 0,
    videoGenerating: 0,
    videoReview: 0,
    completed: 0,
    failed: 0,
  });
}

export function projectCompletionPercent(summary: ProjectPipelineSummary): number {
  if (!summary.total) return 0;
  return Math.round((summary.completed / summary.total) * 100);
}

export function projectPipelineStageCount(
  summary: ProjectPipelineSummary,
  stage: ProjectPipelineStage,
): number {
  switch (stage) {
    case "SHOTS": return summary.total;
    case "IMAGE_CONFIGURED": return summary.imageConfigured;
    case "IMAGE_GENERATING": return summary.imageGenerating;
    case "IMAGE_REVIEW": return summary.imageReview;
    case "IMAGE_SELECTED": return summary.imageSelected;
    case "VIDEO_CONFIGURED": return summary.videoConfigured;
    case "VIDEO_GENERATING": return summary.videoGenerating;
    case "VIDEO_REVIEW": return summary.videoReview;
    case "COMPLETED": return summary.completed;
  }
}

export function reviewShotIds(shots: ShotView[], stage: ShotStage): string[] {
  const reviewStatus = stage === "image" ? "IMAGE_REVIEW" : "VIDEO_REVIEW";
  return shots
    .filter((shot) => deriveStageStatus(shot, stage) === reviewStatus)
    .sort((left, right) => left.ordinal - right.ordinal || left.id.localeCompare(right.id))
    .map((shot) => shot.id);
}

export interface ProjectProductionPipelineProps extends ShotBulkConfigPanelProps {
  onOpenReview?: (stage: ShotStage, shotIds: string[]) => void | Promise<void>;
}

const METRICS: ReadonlyArray<{
  key: keyof ProjectPipelineSummary;
  label: string;
  tone: string;
}> = [
  { key: "total", label: "Total Shots", tone: "neutral" },
  { key: "unconfigured", label: "Unconfigured", tone: "warning" },
  { key: "imageReady", label: "Image Ready", tone: "muted" },
  { key: "imageGenerating", label: "Image Generating", tone: "active" },
  { key: "imageReview", label: "Image Review", tone: "review" },
  { key: "imageSelected", label: "Image Selected", tone: "success" },
  { key: "videoReady", label: "Video Ready", tone: "muted" },
  { key: "videoGenerating", label: "Video Generating", tone: "active" },
  { key: "videoReview", label: "Video Review", tone: "review" },
  { key: "completed", label: "Completed", tone: "success" },
  { key: "failed", label: "Failed", tone: "danger" },
];

const STAGE_LABELS: Record<ProjectPipelineStage, string> = {
  SHOTS: "Shots",
  IMAGE_CONFIGURED: "Image Configured",
  IMAGE_GENERATING: "Image Generating",
  IMAGE_REVIEW: "Image Review",
  IMAGE_SELECTED: "Image Selected",
  VIDEO_CONFIGURED: "Video Configured",
  VIDEO_GENERATING: "Video Generating",
  VIDEO_REVIEW: "Video Review",
  COMPLETED: "Completed",
};

export function ProjectProductionPipeline({ onOpenReview, ...bulkProps }: ProjectProductionPipelineProps) {
  const summary = useMemo(() => deriveProjectPipelineSummary(bulkProps.shots), [bulkProps.shots]);
  const completionPercent = projectCompletionPercent(summary);
  const imageReviewIds = useMemo(() => reviewShotIds(bulkProps.shots, "image"), [bulkProps.shots]);
  const videoReviewIds = useMemo(() => reviewShotIds(bulkProps.shots, "video"), [bulkProps.shots]);

  async function openReview(stage: ShotStage, shotIds: string[]) {
    if (!onOpenReview || !shotIds.length) return;
    try {
      await onOpenReview(stage, shotIds);
    } catch (error: unknown) {
      bulkProps.onError?.(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <section className="project-production-pipeline" aria-label="项目生产管线">
      <div className="pipeline-section-heading">
        <div>
          <span className="section-label">Project Production</span>
          <h2>项目生产管线</h2>
          <p className="pipeline-muted">进度由当前 Shot、阶段配置、任务和人工选择实时派生，不保存容易过期的项目进度字段。</p>
        </div>
        <strong className="pipeline-completion-count">{summary.completed} / {summary.total} Completed · {completionPercent}%</strong>
      </div>

      <ShotProgressDashboard shots={bulkProps.shots} />

      <div className="pipeline-summary-grid" aria-label="项目生产状态汇总">
        {METRICS.map((metric) => (
          <div key={metric.key} className={`pipeline-summary-metric pipeline-summary-metric-${metric.tone}`}>
            <span>{metric.label}</span>
            <strong>{summary[metric.key]}</strong>
          </div>
        ))}
      </div>

      <div className="pipeline-progress-track" aria-label={`项目完成度 ${completionPercent}%`}>
        <span style={{ width: `${completionPercent}%` }} />
      </div>

      <ol className="pipeline-stage-list" aria-label="项目生产阶段">
        {PROJECT_PIPELINE_STAGES.map((stage) => {
          const count = projectPipelineStageCount(summary, stage);
          const isReview = stage === "IMAGE_REVIEW" || stage === "VIDEO_REVIEW";
          return (
            <li key={stage} className={`pipeline-stage pipeline-stage-${isReview ? "review" : "default"}`}>
              <span className="pipeline-stage-index">{PROJECT_PIPELINE_STAGES.indexOf(stage) + 1}</span>
              <span className="pipeline-stage-copy"><strong>{STAGE_LABELS[stage]}</strong><small>{count} / {summary.total}</small></span>
              {isReview && <span className="pipeline-stage-review-badge">人工选择</span>}
            </li>
          );
        })}
      </ol>

      <div className="pipeline-review-actions">
        <div>
          <strong>Review 仍由人完成</strong>
          <p className="pipeline-muted">生成结果不会自动成为 selected asset；REF2VA 的 ordered references 也继续由用户配置。</p>
        </div>
        <div className="pipeline-action-grid">
          <button type="button" onClick={() => void openReview("image", imageReviewIds)} disabled={!onOpenReview || !imageReviewIds.length}>Review 图片 ({imageReviewIds.length})</button>
          <button type="button" onClick={() => void openReview("video", videoReviewIds)} disabled={!onOpenReview || !videoReviewIds.length}>Review 视频 ({videoReviewIds.length})</button>
        </div>
      </div>

      <ShotBulkConfigPanel {...bulkProps} />
    </section>
  );
}
