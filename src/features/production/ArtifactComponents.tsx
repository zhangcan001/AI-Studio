import { useEffect, useState } from "react";
import { getAssetMediaUrl, openArtifact, readAssetImage, revealArtifact } from "../../services/tauriClient";
import type { ArtifactDto, ArtifactReviewDecision } from "../../types/artifact";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, formatDurationMs, formatFileSize } from "../../i18n/statusLabels";
import "./ArtifactComponents.css";

export function ArtifactPreview({ projectId, artifact }: { projectId: string; artifact: ArtifactDto }) {
  const [imageUrl, setImageUrl] = useState<string>();
  const [error, setError] = useState<string>();

  useEffect(() => {
    setImageUrl(undefined);
    setError(undefined);
    if (artifact.availability !== "available" || artifact.mediaType !== "image") return undefined;
    let active = true;
    let objectUrl: string | undefined;
    void readAssetImage(projectId, artifact.id)
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: artifact.mimeType }));
        setImageUrl(objectUrl);
      })
      .catch((cause: unknown) => {
        if (active) setError(toUserMessage(cause));
      });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [artifact.availability, artifact.id, artifact.mediaType, artifact.mimeType, projectId]);

  if (artifact.availability === "missing") {
    return <div className="artifact-preview-placeholder" role="status">文件不存在，无法预览或打开。</div>;
  }
  if (artifact.availability === "unavailable") {
    return <div className="artifact-preview-placeholder" role="status">产物位置不可用，已阻止访问。</div>;
  }

  if (artifact.mediaType === "video") {
    return <video className="artifact-preview-media" src={getAssetMediaUrl(projectId, artifact.id, "video")} controls preload="metadata" playsInline aria-label={artifact.name} />;
  }
  if (artifact.mediaType === "audio") {
    return <audio className="artifact-preview-audio" src={getAssetMediaUrl(projectId, artifact.id, "audio")} controls preload="metadata" aria-label={artifact.name} />;
  }
  if (artifact.mediaType === "image") {
    return imageUrl
      ? <img className="artifact-preview-media" src={imageUrl} alt={artifact.name} />
      : <div className="artifact-preview-placeholder" role={error ? "alert" : "status"}>{error ?? "正在加载产物预览…"}</div>;
  }
  return <div className="artifact-preview-placeholder" role="status">此类型暂不支持预览，可打开原文件查看。</div>;
}

export function ArtifactActions({ artifact }: { artifact: ArtifactDto }) {
  const [busy, setBusy] = useState<"open" | "reveal">();
  const [error, setError] = useState<string>();
  const available = artifact.availability === "available";

  async function run(action: "open" | "reveal") {
    if (!available || busy) return;
    setBusy(action);
    setError(undefined);
    try {
      if (action === "open") await openArtifact(artifact.id);
      else await revealArtifact(artifact.id);
    } catch (cause: unknown) {
      setError(toUserMessage(cause));
    } finally {
      setBusy(undefined);
    }
  }

  return (
    <div className="artifact-actions">
      <button type="button" onClick={() => void run("open")} disabled={!available || Boolean(busy)}>
        {busy === "open" ? "正在打开…" : "打开"}
      </button>
      <button type="button" onClick={() => void run("reveal")} disabled={!available || Boolean(busy)}>
        {busy === "reveal" ? "正在定位…" : "在文件夹中显示"}
      </button>
      {error && <p role="alert" className="artifact-action-error">{error}</p>}
    </div>
  );
}

export function ArtifactMetadata({ artifact }: { artifact: ArtifactDto }) {
  const dimensions = artifact.width && artifact.height ? `${artifact.width} × ${artifact.height}` : undefined;
  const duration = artifact.durationMs != null ? formatDurationMs(artifact.durationMs) : undefined;
  const availabilityLabel = artifact.availability === "available"
    ? "可用"
    : artifact.availability === "missing" ? "文件不存在" : "文件位置不可用";
  return (
    <dl className="artifact-metadata">
      <div><dt>Artifact ID</dt><dd><code title={artifact.id}>{artifact.id}</code></dd></div>
      <div><dt>文件状态</dt><dd><span className={`artifact-availability artifact-availability-${artifact.availability}`}>{availabilityLabel}</span></dd></div>
      <div><dt>类型</dt><dd>{artifact.mediaType.toUpperCase()}</dd></div>
      <div><dt>大小</dt><dd>{formatFileSize(artifact.sizeBytes)}</dd></div>
      {(dimensions || duration) && <div><dt>媒体信息</dt><dd>{dimensions ?? duration}{dimensions && duration ? ` · ${duration}` : ""}</dd></div>}
      <div><dt>版本</dt><dd>v{artifact.version}</dd></div>
      <div><dt>输出</dt><dd>{artifact.outputId} · #{artifact.ordinal + 1}</dd></div>
      <div><dt>创建时间</dt><dd>{formatDateTime(artifact.createdAt)}</dd></div>
      <div><dt>任务</dt><dd>{artifact.taskId}</dd></div>
    </dl>
  );
}

const REVIEW_LABELS: Record<ArtifactReviewDecision | "UNREVIEWED", string> = {
  PENDING: "待审核",
  APPROVED: "已通过",
  REJECTED: "已驳回",
  UNREVIEWED: "无审核记录",
};

export function ReviewBadge({ decision }: { decision?: ArtifactReviewDecision }) {
  const value = decision ?? "UNREVIEWED";
  return <span className={`artifact-review-badge artifact-review-${value.toLowerCase()}`}>{REVIEW_LABELS[value]}</span>;
}
