import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getProductionBatchReview,
  readAssetThumbnail,
  regenerateMarkedProductionItems,
  regenerateProductionItem,
  setProductionReviewNote,
  setProductionReviewStatus,
} from "../../services/tauriClient";
import { getAssetMediaUrl } from "../../services/tauriClient";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, productionItemStatusLabel, taskStatusLabel } from "../../i18n/statusLabels";
import { MINIMAX_H3_RESOLUTION_PRESETS } from "../runtime/resolutionPresets";
import type { ProductionBatchReview, ProductionReviewItem, ProductionReviewStatus } from "../../types/productionItemReview";

interface Props {
  projectId: string;
  batchId: string;
  refreshKey?: string;
  onOpenTask: (taskId: string) => void;
  onBatchChanged?: () => Promise<void>;
}

type Filter = "ALL" | ProductionReviewStatus;

const FILTERS: Array<{ value: Filter; label: string }> = [
  { value: "ALL", label: "全部" },
  { value: "UNREVIEWED", label: "未审" },
  { value: "APPROVED", label: "通过" },
  { value: "STARRED", label: "优秀" },
  { value: "REGENERATE", label: "待重生成" },
  { value: "REJECTED", label: "废弃" },
  { value: "FAILED", label: "生成失败" },
];

const REVIEW_LABELS: Record<ProductionReviewStatus, string> = {
  UNREVIEWED: "未审",
  APPROVED: "通过",
  STARRED: "优秀",
  REGENERATE: "待重生成",
  REJECTED: "废弃",
  FAILED: "生成失败",
  IN_PROGRESS: "生成中",
};

const REVIEW_CLASS: Record<ProductionReviewStatus, string> = {
  UNREVIEWED: "review-status-unreviewed",
  APPROVED: "review-status-approved",
  STARRED: "review-status-starred",
  REGENERATE: "review-status-regenerate",
  REJECTED: "review-status-rejected",
  FAILED: "review-status-failed",
  IN_PROGRESS: "review-status-in-progress",
};

export function ProductionBatchReviewWorkspace({ projectId, batchId, refreshKey, onOpenTask, onBatchChanged }: Props) {
  const [review, setReview] = useState<ProductionBatchReview>();
  const [filter, setFilter] = useState<Filter>("ALL");
  const [expandedId, setExpandedId] = useState<string>();
  const [noteDrafts, setNoteDrafts] = useState<Record<string, string>>({});
  const [regenerateItem, setRegenerateItem] = useState<ProductionReviewItem>();
  const [busyAction, setBusyAction] = useState<string>();
  const [itemErrors, setItemErrors] = useState<Record<string, string>>({});
  const [notice, setNotice] = useState<string>();

  const refresh = useCallback(async () => {
    try {
      const next = await getProductionBatchReview(projectId, batchId);
      setReview(next);
      setNoteDrafts(Object.fromEntries(next.items.map((item) => [item.itemId, item.reviewNote])));
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    }
  }, [batchId, projectId]);

  useEffect(() => {
    setReview(undefined);
    setExpandedId(undefined);
    setRegenerateItem(undefined);
  }, [batchId, projectId]);

  useEffect(() => {
    void refresh();
  }, [refresh, refreshKey]);

  const visibleItems = useMemo(
    () => review?.items.filter((item) => filter === "ALL" || item.reviewStatus === filter) ?? [],
    [filter, review],
  );

  async function changeStatus(item: ProductionReviewItem, status: Exclude<ProductionReviewStatus, "FAILED" | "IN_PROGRESS">) {
    setBusyAction(`status:${item.itemId}`);
    setNotice(undefined);
    setItemErrors((current) => ({ ...current, [item.itemId]: "" }));
    try {
      setReview(await setProductionReviewStatus({ projectId, batchId, itemId: item.itemId, status }));
    } catch (error: unknown) {
      setItemErrors((current) => ({ ...current, [item.itemId]: toUserMessage(error) }));
    } finally {
      setBusyAction(undefined);
    }
  }

  async function saveNote(item: ProductionReviewItem) {
    setBusyAction(`note:${item.itemId}`);
    setNotice(undefined);
    setItemErrors((current) => ({ ...current, [item.itemId]: "" }));
    try {
      setReview(await setProductionReviewNote({
        projectId,
        batchId,
        itemId: item.itemId,
        note: noteDrafts[item.itemId] ?? "",
      }));
    } catch (error: unknown) {
      setItemErrors((current) => ({ ...current, [item.itemId]: toUserMessage(error) }));
    } finally {
      setBusyAction(undefined);
    }
  }

  async function regenerateMarked() {
    setBusyAction("bulk-regenerate");
    setNotice(undefined);
    try {
      const result = await regenerateMarkedProductionItems({ projectId, batchId, autoStart: true });
      setNotice(result.autoStarted ? `已创建返工批次，共 ${result.selectedCount} 项，并开始生成。` : `已创建返工批次，共 ${result.selectedCount} 项。`);
      await onBatchChanged?.();
      await refresh();
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusyAction(undefined);
    }
  }

  async function submitRegenerate(request: {
    promptOverride?: string;
    durationSeconds?: number;
    width?: number;
    height?: number;
    useOriginalSeed: boolean;
  }) {
    if (!regenerateItem) return;
    setBusyAction(`regenerate:${regenerateItem.itemId}`);
    setNotice(undefined);
    try {
      const result = await regenerateProductionItem({
        projectId,
        batchId,
        itemId: regenerateItem.itemId,
        ...request,
        autoStart: true,
      });
      setRegenerateItem(undefined);
      setNotice(result.autoStarted ? "已创建返工批次并开始生成。" : "已创建返工批次。" );
      await onBatchChanged?.();
      await refresh();
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusyAction(undefined);
    }
  }

  if (!review) {
    return <section className="production-review-workspace"><p className="disabled-note">正在加载审片结果…</p></section>;
  }

  return (
    <section className="production-review-workspace" aria-label="H3 批次审片">
      <div className="production-review-heading">
        <div>
          <span className="section-label">H3 批量审片</span>
          <h3>生成结果审核与局部返工</h3>
          <p>审核状态独立于 Task；返工从已冻结参数创建新版本，不重新扫描项目文件夹。</p>
        </div>
        <button
          type="button"
          className="quiet-button"
          onClick={() => void regenerateMarked()}
          disabled={busyAction !== undefined || review.regenerateCount === 0}
        >
          {busyAction === "bulk-regenerate" ? "正在创建返工…" : `重生成全部待返工（${review.regenerateCount}）`}
        </button>
      </div>

      <div className="production-review-stats" aria-label="审片统计">
        <ReviewStat label="总计" value={review.total} />
        <ReviewStat label="成功" value={review.successCount} />
        <ReviewStat label="失败" value={review.failedCount} />
        <ReviewStat label="未审" value={review.unreviewedCount} />
        <ReviewStat label="通过" value={review.approvedCount} />
        <ReviewStat label="优秀" value={review.starredCount} />
        <ReviewStat label="待重生成" value={review.regenerateCount} />
        <ReviewStat label="废弃" value={review.rejectedCount} />
      </div>

      <div className="production-review-filters" role="toolbar" aria-label="审片筛选">
        {FILTERS.map((option) => (
          <button
            key={option.value}
            type="button"
            className={filter === option.value ? "review-filter-active" : "quiet-button"}
            aria-pressed={filter === option.value}
            onClick={() => setFilter(option.value)}
          >
            {option.label}
          </button>
        ))}
      </div>

      <div className="production-review-list">
        {visibleItems.length ? visibleItems.map((item) => (
          <ReviewCard
            key={item.itemId}
            projectId={projectId}
            item={item}
            expanded={expandedId === item.itemId}
            note={noteDrafts[item.itemId] ?? item.reviewNote}
            error={itemErrors[item.itemId]}
            busy={busyAction !== undefined}
            onToggle={() => setExpandedId((current) => current === item.itemId ? undefined : item.itemId)}
            onNoteChange={(note) => setNoteDrafts((current) => ({ ...current, [item.itemId]: note }))}
            onSaveNote={() => void saveNote(item)}
            onStatus={(status) => void changeStatus(item, status)}
            onRegenerate={() => setRegenerateItem(item)}
            onOpenTask={onOpenTask}
          />
        )) : <p className="disabled-note">当前筛选没有结果。</p>}
      </div>

      {notice && <p className="studio-notice" role="status">{notice}</p>}
      {regenerateItem && (
        <RegenerateDialog
          item={regenerateItem}
          busy={busyAction === `regenerate:${regenerateItem.itemId}`}
          onClose={() => setRegenerateItem(undefined)}
          onSubmit={(request) => void submitRegenerate(request)}
        />
      )}
    </section>
  );
}

function ReviewStat({ label, value }: { label: string; value: number }) {
  return <div className="production-review-stat"><span>{label}</span><strong>{value}</strong></div>;
}

function ReviewCard({
  projectId,
  item,
  expanded,
  note,
  error,
  busy,
  onToggle,
  onNoteChange,
  onSaveNote,
  onStatus,
  onRegenerate,
  onOpenTask,
}: {
  projectId: string;
  item: ProductionReviewItem;
  expanded: boolean;
  note: string;
  error?: string;
  busy: boolean;
  onToggle: () => void;
  onNoteChange: (note: string) => void;
  onSaveNote: () => void;
  onStatus: (status: Exclude<ProductionReviewStatus, "FAILED" | "IN_PROGRESS">) => void;
  onRegenerate: () => void;
  onOpenTask: (taskId: string) => void;
}) {
  const asset = item.outputAssets.find((candidate) => candidate.assetType === "video" || candidate.category === "generated_video");
  const reviewDisabled = item.reviewStatus === "FAILED" || item.reviewStatus === "IN_PROGRESS";
  const posterUrl = useAssetThumbnailUrl(projectId, asset?.id);
  return (
    <article className={`production-review-card${expanded ? " production-review-card-expanded" : ""}`}>
      <button type="button" className="production-review-card-toggle" onClick={onToggle} aria-expanded={expanded}>
        <span className="production-review-ordinal">#{String(item.ordinal + 1).padStart(2, "0")}</span>
        <span
          className="production-review-card-preview"
          aria-hidden="true"
          style={posterUrl ? { backgroundImage: `linear-gradient(rgb(5 10 18 / 24%), rgb(5 10 18 / 66%)), url(${posterUrl})` } : undefined}
        >
          {asset ? "▶" : "—"}
        </span>
        <span className="production-review-card-summary">
          <span className="production-review-card-title">
            <strong>{item.version ? `V${item.version}` : "—"}</strong>
            <span className={`review-status ${REVIEW_CLASS[item.reviewStatus]}`}>{REVIEW_LABELS[item.reviewStatus]}</span>
            {item.preferred && <span className="review-preferred">当前优选</span>}
          </span>
          <span className="production-review-prompt">{item.promptText || "未提取 Prompt"}</span>
          <small>{item.qualityProfile} · {item.durationSeconds ? `${item.durationSeconds} 秒` : "—"} · {item.width && item.height ? `${item.width} × ${item.height}` : "—"} · {taskStatusLabel(item.taskStatus)}</small>
        </span>
        <span className="production-review-card-chevron" aria-hidden="true">{expanded ? "收起" : "查看"}</span>
      </button>

      {expanded && (
        <div className="production-review-card-detail">
          {asset ? <ReviewVideo projectId={projectId} assetId={asset.id} name={asset.name} posterUrl={posterUrl} /> : <div className="production-review-video-empty">{productionItemStatusLabel(item.productionItemStatus)}</div>}
          <div className="production-review-metadata">
            <span>生成模式：{modeLabel(item.workflowVersionId)}</span>
            <span>Task：{item.taskId ?? "尚未创建"}</span>
            <span>创建：{formatDateTime(item.createdAt)}</span>
            {item.finishedAt && <span>完成：{formatDateTime(item.finishedAt)}</span>}
            {item.seed && <span>Seed：{item.seed}</span>}
          </div>
          <p className="production-review-prompt-full">{item.promptText || "未提取 Prompt"}</p>
          {error && <p className="production-review-item-error" role="alert">{error}</p>}
          <label className="production-review-note">
            <span>审片备注</span>
            <textarea value={note} maxLength={4096} rows={3} onChange={(event) => onNoteChange(event.target.value)} placeholder="例如：镜头太快、脸型漂了、这版通过……" disabled={busy || reviewDisabled} />
            <button type="button" className="quiet-button" onClick={onSaveNote} disabled={busy || reviewDisabled}>保存备注</button>
          </label>
          <div className="production-review-actions">
            <button type="button" onClick={() => onStatus("APPROVED")} disabled={busy || reviewDisabled}>通过</button>
            <button type="button" className="review-star-button" onClick={() => onStatus("STARRED")} disabled={busy || reviewDisabled}>优秀</button>
            <button type="button" className="quiet-button" onClick={() => onStatus("REGENERATE")} disabled={busy || reviewDisabled}>待重生成</button>
            <button type="button" className="quiet-button danger-button" onClick={() => onStatus("REJECTED")} disabled={busy || reviewDisabled}>废弃</button>
            {item.taskId && <button type="button" className="quiet-button" onClick={() => onOpenTask(item.taskId!)}>查看 Task</button>}
            {!reviewDisabled && <button type="button" className="quiet-button" onClick={onRegenerate} disabled={busy}>编辑并重生成</button>}
          </div>
        </div>
      )}
    </article>
  );
}

function useAssetThumbnailUrl(projectId: string, assetId?: string) {
  const [posterUrl, setPosterUrl] = useState<string>();
  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    setPosterUrl(undefined);
    if (!assetId) return () => { active = false; };
    void readAssetThumbnail(projectId, assetId)
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setPosterUrl(objectUrl);
      })
      .catch(() => {
        if (active) setPosterUrl(undefined);
      });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [assetId, projectId]);
  return posterUrl;
}

function ReviewVideo({ projectId, assetId, name, posterUrl }: { projectId: string; assetId: string; name: string; posterUrl?: string }) {
  const [playing, setPlaying] = useState(false);
  const mediaUrl = getAssetMediaUrl(projectId, assetId, "video");
  return (
    <div className="production-review-video">
      {playing ? <video src={mediaUrl} poster={posterUrl} controls autoPlay preload="metadata" playsInline aria-label={name} /> : <button type="button" className="production-review-video-placeholder" style={posterUrl ? { backgroundImage: `linear-gradient(rgb(5 10 18 / 28%), rgb(5 10 18 / 72%)), url(${posterUrl})`, backgroundPosition: "center", backgroundSize: "cover" } : undefined} onClick={() => setPlaying(true)}><span aria-hidden="true">▶</span><span>点击播放视频</span></button>}
    </div>
  );
}

function RegenerateDialog({
  item,
  busy,
  onClose,
  onSubmit,
}: {
  item: ProductionReviewItem;
  busy: boolean;
  onClose: () => void;
  onSubmit: (request: { promptOverride?: string; durationSeconds?: number; width?: number; height?: number; useOriginalSeed: boolean }) => void;
}) {
  const [prompt, setPrompt] = useState(item.promptText ?? "");
  const [duration, setDuration] = useState(String(item.durationSeconds ?? 5));
  const [resolution, setResolution] = useState(`${item.width ?? 960}x${item.height ?? 544}`);
  const [useOriginalSeed, setUseOriginalSeed] = useState(false);
  const [overridePrompt, setOverridePrompt] = useState(false);
  const selected = MINIMAX_H3_RESOLUTION_PRESETS.find((preset) => `${preset.width}x${preset.height}` === resolution)
    ?? MINIMAX_H3_RESOLUTION_PRESETS[3];
  return (
    <div className="production-review-dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="production-review-dialog" role="dialog" aria-modal="true" aria-label="编辑并重生成" onMouseDown={(event) => event.stopPropagation()}>
        <div className="production-review-dialog-heading"><div><span className="section-label">局部返工</span><h3>编辑并重生成</h3></div><button type="button" className="quiet-button" onClick={onClose}>关闭</button></div>
        <label className="production-review-dialog-checkbox"><input type="checkbox" checked={overridePrompt} onChange={(event) => setOverridePrompt(event.target.checked)} /> 修改 Prompt</label>
        <textarea rows={5} value={prompt} onChange={(event) => setPrompt(event.target.value)} disabled={!overridePrompt || busy} aria-label="重生成 Prompt" />
        <div className="production-review-dialog-grid">
          <label>时长（1–15 秒）<input type="number" min={1} max={15} value={duration} onChange={(event) => setDuration(event.target.value)} disabled={busy} /></label>
          <label>分辨率<select value={resolution} onChange={(event) => setResolution(event.target.value)} disabled={busy}>{MINIMAX_H3_RESOLUTION_PRESETS.map((preset) => <option key={preset.id} value={`${preset.width}x${preset.height}`}>{preset.width} × {preset.height}</option>)}</select></label>
        </div>
        <label className="production-review-dialog-checkbox"><input type="checkbox" checked={useOriginalSeed} onChange={(event) => setUseOriginalSeed(event.target.checked)} disabled={busy} /> 使用原 Seed（默认使用新随机 Seed）</label>
        <p className="disabled-note">生成模式、参考素材、质量档位和 Recipe 继承原版本；不会重新扫描本地文件夹。</p>
        <div className="production-review-dialog-actions"><button type="button" className="quiet-button" onClick={onClose} disabled={busy}>取消</button><button type="button" onClick={() => onSubmit({ promptOverride: overridePrompt ? prompt : undefined, durationSeconds: Number(duration), width: selected.width, height: selected.height, useOriginalSeed })} disabled={busy || !prompt.trim()}>{busy ? "正在创建…" : "创建返工任务"}</button></div>
      </section>
    </div>
  );
}

function modeLabel(workflowVersionId: string): string {
  const value = workflowVersionId.toLowerCase();
  if (value.includes("first_last")) return "首尾帧";
  if (value.includes("reference") || value.includes("ref2va")) return "参考素材";
  if (value.includes("i2v") || value.includes("image")) return "图生视频";
  return "文生视频";
}
