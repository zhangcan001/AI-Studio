import { useEffect, useMemo, useState } from "react";
import {
  getAsset,
  getAssetMediaUrl,
  getProductionBatchReviewProductivity,
  readAssetImage,
  readAssetThumbnail,
  regenerateProductionItem,
  selectShotResult,
  setProductionReviewNote,
  setProductionReviewStatus,
} from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type {
  ProductionBatchReviewProductivity,
  ProductionReviewProductivityItem,
} from "../../services/tauriClient";
import type { ReviewCompareCandidate, ReviewCompareItem, ReviewCompareReferenceAsset, ReviewCompareReferenceSet } from "../../types/reviewProductivity";
import type { ShotStage, ShotView } from "../../types/shot";
import { deriveStageStatus, recentShotFailure, shotStatusLabels } from "./shotDomain";
import { stageLabel } from "./shotBatchDomain";
import { ReviewCompareWorkspace } from "../production/ReviewCompareWorkspace";

interface Props {
  projectId: string;
  shots: ShotView[];
  assets: AssetView[];
  stage: ShotStage;
  busy: boolean;
  onAssetsLoaded: (assets: AssetView[]) => void;
  /** Legacy host callback. It remains authoritative for the fallback board. */
  onSelect: (shotId: string, stage: ShotStage, assetId: string, fromLinkedTask: boolean) => void;
  onRetry: (shotId: string, stage: ShotStage) => void;
  onOpenTask?: (taskId: string) => void;
  /** Optional seam for hosts that can resolve a production review batch. */
  reviewBatchId?: string;
  reviewBatchLoader?: (projectId: string, batchId: string) => Promise<ProductionBatchReviewProductivity>;
  onOpenProductionQueue?: () => void;
}

type ReviewFilter = "ALL" | "NEEDS_REVIEW" | "APPROVED" | "REGENERATE" | "FAILED";

export function ShotBatchReviewBoard({ projectId, shots, assets, stage, busy, onAssetsLoaded, onSelect, onRetry, onOpenTask, reviewBatchId, reviewBatchLoader = getProductionBatchReviewProductivity, onOpenProductionQueue }: Props) {
  const [review, setReview] = useState<ProductionBatchReviewProductivity>();
  const [reviewError, setReviewError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [filter, setFilter] = useState<ReviewFilter>("ALL");
  const [currentReviewItemId, setCurrentReviewItemId] = useState<string>();
  const [localCompareShotId, setLocalCompareShotId] = useState<string>();
  const candidateIds = useMemo(() => [...new Set(shots.flatMap((shot) => shot.generationLinks.filter((link) => link.stage === stage).flatMap((link) => link.task?.outputAssetIds ?? [])))], [shots, stage]);

  useEffect(() => {
    if (reviewBatchId || localCompareShotId) return;
    const missing = candidateIds.filter((id) => !assets.some((asset) => asset.id === id));
    if (!missing.length) return;
    let active = true;
    void Promise.all(missing.slice(0, 100).map((id) => getAsset(projectId, id).catch(() => undefined))).then((loaded) => { if (active) onAssetsLoaded(loaded.filter((asset): asset is AssetView => Boolean(asset))); });
    return () => { active = false; };
  }, [assets, candidateIds, localCompareShotId, onAssetsLoaded, projectId, reviewBatchId]);

  useEffect(() => {
    setReview(undefined); setReviewError(undefined); setCurrentReviewItemId(undefined);
    setLocalCompareShotId(undefined);
    if (!reviewBatchId) return;
    let active = true;
    void reviewBatchLoader(projectId, reviewBatchId).then((next) => { if (!active) return; setReview(next); setCurrentReviewItemId(next.items[0]?.itemId); }).catch(() => { if (active) setReviewError("当前批次没有可用的生产复核上下文，已保留原有批量审片。"); });
    return () => { active = false; };
  }, [projectId, reviewBatchId, reviewBatchLoader]);

  const mappedItems = useMemo(() => review?.items.filter((item) => item.shotId && normalizeStage(item.stage) === stage) ?? [], [review, stage]);
  const detailedCompareAvailable = Boolean(review && mappedItems.length > 0);
  const filteredItems = useMemo(() => mappedItems.filter((item) => matchesFilter(item, filter)), [filter, mappedItems]);
  const failedItems = useMemo(() => (review?.items ?? []).filter((item) => isFailedReviewItem(item) && (normalizeStage(item.stage) === stage || !item.stage)), [review, stage]);
  const currentItem = filteredItems.find((item) => item.itemId === currentReviewItemId) ?? filteredItems[0];
  const reviewImageIds = useMemo(() => reviewImageIdsForItem(currentItem), [currentItem]);
  const [reviewImageUrls, setReviewImageUrls] = useState<Record<string, string>>({});
  useEffect(() => {
    if (!reviewImageIds.length) { setReviewImageUrls({}); return; }
    let active = true;
    const objectUrls: string[] = [];
    void Promise.all(reviewImageIds.map(async (assetId) => {
      const asset = currentItem?.outputAssets.find((candidate) => candidate.id === assetId);
      try {
        const bytes = await (asset?.thumbnailAvailable ? readAssetThumbnail(projectId, assetId).catch(() => readAssetImage(projectId, assetId)) : readAssetImage(projectId, assetId));
        const url = URL.createObjectURL(new Blob([bytes], { type: asset?.mimeType ?? "image/*" }));
        objectUrls.push(url);
        return [assetId, url] as const;
      } catch { return undefined; }
    })).then((loaded) => { if (active) setReviewImageUrls(Object.fromEntries(loaded.filter((entry): entry is readonly [string, string] => Boolean(entry)))); });
    return () => { active = false; objectUrls.forEach((url) => URL.revokeObjectURL(url)); };
  }, [currentItem, projectId, reviewImageIds]);
  const compareItems = useMemo(() => filteredItems.map((item) => toCompareItem(item, projectId, reviewImageUrls)), [filteredItems, projectId, reviewImageUrls]);
  const localCompareItem = useMemo(() => {
    if (reviewBatchId || !localCompareShotId) return undefined;
    const shot = shots.find((item) => item.id === localCompareShotId);
    return shot ? toLocalCompareItem(shot, assets, stage, projectId) : undefined;
  }, [assets, localCompareShotId, projectId, reviewBatchId, shots, stage]);

  async function reloadReview() {
    if (!reviewBatchId) return;
    try { setReview(await reviewBatchLoader(projectId, reviewBatchId)); } catch (error: unknown) { setReviewError(error instanceof Error ? error.message : "审片结果刷新失败。"); }
  }

  async function confirmAndApprove(candidate: ReviewCompareCandidate, item: ReviewCompareItem) {
    if (!reviewBatchId || !item.shotId) return;
    const resultStage = normalizeStage(item.stage);
    if (!resultStage) return;
    setReviewError(undefined);
    try { await selectShotResult({ projectId, shotId: item.shotId, stage: resultStage, assetId: candidate.id, fromLinkedTask: true }); }
    catch (error: unknown) { setReviewError(error instanceof Error ? error.message : "候选选择失败。"); return; }
    try { await setProductionReviewStatus({ projectId, batchId: reviewBatchId, itemId: item.id, status: "APPROVED" }); setNotice("候选已设为采用结果，审片状态已更新为通过。"); await reloadReview(); }
    catch { setReviewError("候选已设为采用结果，但审片状态未更新，请重新点击通过。"); }
  }

  async function setReviewStatus(status: "APPROVED" | "STARRED" | "REJECTED" | "REGENERATE", item: ReviewCompareItem) {
    if (!reviewBatchId) return;
    try { await setProductionReviewStatus({ projectId, batchId: reviewBatchId, itemId: item.id, status }); await reloadReview(); }
    catch (error: unknown) { setReviewError(error instanceof Error ? error.message : "审片状态更新失败。"); }
  }

  async function saveNote(item: ReviewCompareItem, note: string) {
    if (!reviewBatchId) return;
    try { await setProductionReviewNote({ projectId, batchId: reviewBatchId, itemId: item.id, note }); setNotice("审片备注已保存。"); await reloadReview(); }
    catch (error: unknown) { setReviewError(error instanceof Error ? error.message : "审片备注保存失败。"); }
  }

  async function createReworkBatch(item: ReviewCompareItem) {
    if (!reviewBatchId) return;
    try {
      const result = await regenerateProductionItem({ projectId, batchId: reviewBatchId, itemId: item.id, durationSeconds: currentItem?.durationSeconds, width: currentItem?.width, height: currentItem?.height, useOriginalSeed: false, autoStart: false });
      setNotice(`已创建 READY 返工批次（${result.selectedCount} 项），请打开/手动开始队列。`);
      onOpenProductionQueue?.();
      await reloadReview();
    } catch (error: unknown) { setReviewError(error instanceof Error ? error.message : "返工批次创建失败。"); }
  }

  if (localCompareItem) {
    return (
      <section className="shot-batch-review-board" aria-label={`${stageLabel(stage)}本地 A/B 对比`}>
        <div className="shot-block-heading">
          <div><span className="section-label">本地比较</span><h3>{localCompareItem.name} · A/B 对比</h3></div>
          <button type="button" className="quiet-button" onClick={() => setLocalCompareShotId(undefined)}>返回快速审片</button>
        </div>
        <p className="shot-inline-note">本地比较只改变 A/B 槽位，不读取或写入审核状态，也不会选择 Shot。</p>
        <ReviewCompareWorkspace items={[localCompareItem]} actionAvailability={{ confirmAndApprove: false, approve: false, star: false, reject: false, regenerate: false, createReworkBatch: false, saveNote: false }} />
      </section>
    );
  }

  if (detailedCompareAvailable) {
    const counts = reviewCounts(mappedItems);
    return (
      <section className="shot-batch-review-board" aria-label={`${stageLabel(stage)}批量复核`}>
        <div className="shot-block-heading"><div><span className="section-label">人工复核</span><h3>{stage === "image" ? "关键帧候选复核" : "最终视频候选复核"}</h3></div><span className="shot-inline-note">A/B 仅用于比较；只有显式确认并通过才会选择 Shot。</span></div>
        <div className="shot-batch-review-filters" role="toolbar" aria-label="批量复核筛选">
          {([ ["ALL", `全部 ${mappedItems.length}`], ["NEEDS_REVIEW", `待审 ${counts.needsReview}`], ["APPROVED", `已通过 ${counts.approved}`], ["REGENERATE", `待返工 ${counts.regenerate}`], ["FAILED", `失败 ${failedItems.length}`] ] as const).map(([value, label]) => <button key={value} type="button" className={filter === value ? "review-filter-active" : "quiet-button"} aria-pressed={filter === value} onClick={() => setFilter(value)}>{label}</button>)}
        </div>
        {reviewError && <p className="review-compare-error" role="alert">{reviewError}</p>}
        {notice && <p className="studio-notice" role="status">{notice}</p>}
        {compareItems.length ? <ReviewCompareWorkspace
          items={compareItems}
          currentItemId={currentReviewItemId}
          busy={busy}
          error={reviewError}
          actionAvailability={{ confirmAndApprove: Boolean(reviewBatchId), approve: Boolean(reviewBatchId), star: Boolean(reviewBatchId), reject: Boolean(reviewBatchId), regenerate: Boolean(reviewBatchId), createReworkBatch: Boolean(reviewBatchId), saveNote: Boolean(reviewBatchId) }}
          onItemChange={(item) => setCurrentReviewItemId(item.id)}
          onConfirmAndApprove={(candidate, item) => void confirmAndApprove(candidate, item)}
          onApprove={(_candidate, item) => void setReviewStatus("APPROVED", item)}
          onStar={(_candidate, item) => void setReviewStatus("STARRED", item)}
          onReject={(_candidate, item) => void setReviewStatus("REJECTED", item)}
          onRegenerate={(_candidate, item) => void setReviewStatus("REGENERATE", item)}
          onCreateReworkBatch={(_candidate, item) => void createReworkBatch(item)}
          onSaveNote={(item, note) => void saveNote(item, note)}
        /> : <p className="empty-state">当前筛选没有结果。</p>}
        {failedItems.length > 0 && <FailedReviewItems items={failedItems} busy={busy} onRetry={onRetry} onOpenTask={onOpenTask} />}
      </section>
    );
  }
  return <LegacyReviewBoard projectId={projectId} shots={shots} assets={assets} stage={stage} busy={busy} onAssetsLoaded={onAssetsLoaded} onSelect={onSelect} onRetry={onRetry} onOpenTask={onOpenTask} onOpenLocalCompare={(shotId) => setLocalCompareShotId(shotId)} />;
}

function LegacyReviewBoard({ projectId, shots, assets, stage, busy, onAssetsLoaded, onSelect, onRetry, onOpenTask, onOpenLocalCompare }: Omit<Props, "reviewBatchId" | "reviewBatchLoader" | "onOpenProductionQueue"> & { onOpenLocalCompare: (shotId: string) => void }) {
  void onAssetsLoaded;
  return <section className="shot-batch-review-board" aria-label={`${stageLabel(stage)}批量复核`}><div className="shot-block-heading"><div><span className="section-label">人工复核</span><h3>{stage === "image" ? "关键帧候选复核" : "最终视频候选复核"}</h3></div><span className="shot-inline-note">每个镜头必须明确选择结果后才进入下一阶段。</span></div><div className="shot-batch-review-grid">{shots.map((shot) => { const status = deriveStageStatus(shot, stage); const links = shot.generationLinks.filter((link) => link.stage === stage); const ids = new Set(links.flatMap((link) => link.task?.outputAssetIds ?? [])); const candidates = assets.filter((asset) => ids.has(asset.id) && (stage === "image" ? asset.assetType === "image" : asset.assetType === "video")); const selectedId = stage === "image" ? shot.selectedImageAssetId : shot.selectedVideoAssetId; const failure = recentShotFailure(shot, stage); return <article key={shot.id} className="shot-batch-review-card"><div className="shot-batch-review-card-heading"><div><strong>{String(shot.ordinal + 1).padStart(2, "0")} · {shot.name}</strong><small>{shotStatusLabels[status]}</small></div>{selectedId && <span className="shot-batch-selected-badge">已选择</span>}</div>{candidates.length > 0 ? <div className="shot-batch-review-candidates">{candidates.map((asset) => <div key={asset.id} className={`shot-batch-review-candidate${selectedId === asset.id ? " selected" : ""}`}><ReviewAssetMedia projectId={projectId} asset={asset} stage={stage} /><div><strong>{asset.name}</strong><small>{selectedId === asset.id ? "当前选择" : "候选结果"}</small></div><button type="button" disabled={busy || selectedId === asset.id} onClick={() => onSelect(shot.id, stage, asset.id, true)}>{selectedId === asset.id ? "已选" : stage === "image" ? "设为关键帧" : "设为最终视频"}</button></div>)}<button type="button" className="quiet-button" disabled={busy} onClick={() => onOpenLocalCompare(shot.id)}>打开 A/B 对比</button></div> : <p className="empty-state">{status === "GENERATING_IMAGE" || status === "GENERATING_VIDEO" ? "任务运行中，等待结果…" : "暂无候选结果"}</p>}{failure && <div className="shot-batch-review-failure"><strong>最近一次任务失败</strong><span>{failure.error?.message ?? "生成任务失败，需要处理"}</span><div><button type="button" className="quiet-button" disabled={busy} onClick={() => onRetry(shot.id, stage)}>重新加入队列</button>{onOpenTask && <button type="button" className="quiet-button" disabled={busy} onClick={() => failure.id && onOpenTask(failure.id)}>查看任务详情</button>}</div></div>}</article>; })}{!shots.length && <p className="empty-state">还没有镜头。</p>}</div></section>;
}

function FailedReviewItems({ items, busy, onRetry, onOpenTask }: { items: ProductionReviewProductivityItem[]; busy: boolean; onRetry: Props["onRetry"]; onOpenTask?: Props["onOpenTask"] }) {
  return <section className="shot-batch-review-failure" aria-label="失败复核项"><strong>失败项（{items.length}）</strong>{items.map((item) => <div key={item.itemId}><span>{item.itemId} · 生成任务失败，需要处理</span><div>{item.shotId && item.stage && <button type="button" className="quiet-button" disabled={busy} onClick={() => onRetry(item.shotId!, normalizeStage(item.stage) ?? "image")}>重新加入队列</button>}{onOpenTask && item.taskId && <button type="button" className="quiet-button" disabled={busy} onClick={() => onOpenTask(item.taskId!)}>查看任务详情</button>}</div></div>)}</section>;
}

function ReviewAssetMedia({ projectId, asset, stage }: { projectId: string; asset: AssetView; stage: ShotStage }) {
  const [url, setUrl] = useState<string>();
  useEffect(() => { if (stage === "video") return undefined; let active = true; let objectUrl: string | undefined; const load = asset.thumbnailAvailable ? readAssetThumbnail(projectId, asset.id).catch(() => readAssetImage(projectId, asset.id)) : readAssetImage(projectId, asset.id); void load.then((bytes) => { if (!active) return; objectUrl = URL.createObjectURL(new Blob([bytes], { type: asset.mimeType })); setUrl(objectUrl); }).catch(() => undefined); return () => { active = false; if (objectUrl) URL.revokeObjectURL(objectUrl); }; }, [asset.id, asset.mimeType, asset.thumbnailAvailable, projectId, stage]);
  if (stage === "video") return <video className="shot-batch-review-media" src={getAssetMediaUrl(projectId, asset.id, "video")} controls preload="metadata" playsInline aria-label={asset.name} />;
  return <div className="shot-batch-review-thumb">{url ? <img src={url} alt={asset.name} /> : <span>图片</span>}</div>;
}

export function toCompareItem(item: ProductionReviewProductivityItem, projectId: string, imageUrls: Record<string, string> = {}): ReviewCompareItem {
  const candidates = item.candidateAssets.map((candidate): ReviewCompareCandidate => ({ id: candidate.assetId, itemId: item.itemId, label: candidate.name, asset: item.outputAssets.find((asset) => asset.id === candidate.assetId) ?? placeholderAsset(candidate), mediaKind: assetMediaKind(candidate.assetType), imageUrl: imageUrls[candidate.assetId], mediaUrl: candidate.assetType.toLowerCase().includes("video") ? getAssetMediaUrl(projectId, candidate.assetId, "video") : undefined, selected: candidate.selected, reviewStatus: candidate.reviewResult as ReviewCompareCandidate["reviewStatus"], productionItemStatus: item.productionItemStatus, taskStatus: item.taskStatus, reviewNote: item.reviewNote, historicalContext: toCompareContext(item) }));
  return { id: item.itemId, ordinal: item.ordinal, name: item.itemId, shotId: item.shotId, shotName: item.shotId, stage: normalizeStage(item.stage) ?? "image", candidates, selectedCandidateId: item.selectedAssetId, reviewStatus: item.reviewStatus, reviewNote: item.reviewNote, contextSnapshot: toCompareContext(item) };
}

export function toLocalCompareItem(shot: ShotView, assets: AssetView[], stage: ShotStage, projectId: string): ReviewCompareItem {
  const linkIds = new Set(shot.generationLinks.filter((link) => link.stage === stage).flatMap((link) => link.task?.outputAssetIds ?? []));
  const selectedCandidateId = stage === "image" ? shot.selectedImageAssetId : shot.selectedVideoAssetId;
  return {
    id: `local:${shot.id}:${stage}`,
    ordinal: shot.ordinal,
    name: shot.name,
    shotId: shot.id,
    stage,
    selectedCandidateId,
    candidates: assets.filter((asset) => linkIds.has(asset.id) && (stage === "image" ? asset.assetType === "image" : asset.assetType === "video")).map((asset): ReviewCompareCandidate => ({
      id: asset.id,
      label: asset.name,
      asset,
      mediaKind: stage,
      mediaUrl: stage === "video" ? getAssetMediaUrl(projectId, asset.id, "video") : undefined,
      selected: selectedCandidateId === asset.id,
    })),
  };
}

export function reviewImageIdsForItem(item?: ProductionReviewProductivityItem): string[] {
  if (!item) return [];
  return [...new Set(item.candidateAssets.filter((candidate) => assetMediaKind(candidate.assetType) === "image").map((candidate) => candidate.assetId))].slice(0, 100);
}

function toCompareContext(item: ProductionReviewProductivityItem) {
  const context = item.context ?? {};
  const raw = context as unknown as { referenceSets?: unknown[]; referenceAssets?: unknown[] };
  return { ...context, source: context.snapshotAvailable ? "snapshot" : "legacy", currentName: item.shotId, prompt: context.promptText, workflow: context.workflowVersionId, recipe: context.recipeId, readiness: context.readinessStatus, referenceSets: normalizeReferenceSets(raw.referenceSets), referenceAssets: normalizeReferenceAssets(raw.referenceAssets) };
}

function normalizeReferenceSets(values: unknown[] | undefined): ReviewCompareReferenceSet[] {
  return (values ?? []).map((value) => {
    const raw = value as Record<string, unknown>;
    return { id: String(raw.id ?? raw.referenceSetId ?? ""), name: String(raw.name ?? raw.referenceSetId ?? ""), assets: [] };
  });
}

function normalizeReferenceAssets(values: unknown[] | undefined): ReviewCompareReferenceAsset[] {
  return (values ?? []).map((value) => {
    const raw = value as Record<string, unknown>;
    return {
      id: String(raw.id ?? raw.assetId ?? ""),
      name: String(raw.name ?? raw.assetId ?? ""),
      ...(typeof raw.sha256 === "string" ? { sha256: raw.sha256 } : {}),
      ...(typeof raw.role === "string" ? { role: raw.role } : {}),
      ...(typeof raw.ordinal === "number" ? { ordinal: raw.ordinal } : {}),
      ...(typeof raw.sourceReferenceSetId === "string" ? { sourceReferenceSetId: raw.sourceReferenceSetId } : {}),
    } as ReviewCompareReferenceAsset;
  });
}

function placeholderAsset(candidate: { assetId: string; assetType: string; name: string; mimeType: string; width?: number; height?: number; thumbnailAvailable: boolean }): AssetView {
  return { id: candidate.assetId, assetType: candidate.assetType, category: candidate.assetType === "video" ? "generated_video" : "generated_image", name: candidate.name, originalName: candidate.name, mimeType: candidate.mimeType, width: candidate.width, height: candidate.height, fileSize: 0, createdAt: "", thumbnailAvailable: candidate.thumbnailAvailable, isFavorite: false, tags: [] };
}

export function normalizeStage(value?: string): ShotStage | undefined { if (value?.toLowerCase() === "image") return "image"; if (value?.toLowerCase() === "video") return "video"; return undefined; }
function assetMediaKind(value: string): "image" | "video" { return value.toLowerCase().includes("video") ? "video" : "image"; }
function isFailedReviewItem(item: ProductionReviewProductivityItem): boolean { return item.productionItemStatus === "FAILED" || item.taskStatus === "FAILED" || item.reviewStatus === "FAILED"; }
export function matchesFilter(item: ProductionReviewProductivityItem, filter: ReviewFilter): boolean { if (filter === "FAILED") return isFailedReviewItem(item); if (filter === "APPROVED") return item.reviewStatus === "APPROVED"; if (filter === "REGENERATE") return item.reviewStatus === "REGENERATE"; if (filter === "NEEDS_REVIEW") return item.reviewStatus === "UNREVIEWED"; return true; }
export function reviewCounts(items: ProductionReviewProductivityItem[]) { return { needsReview: items.filter((item) => item.reviewStatus === "UNREVIEWED").length, approved: items.filter((item) => item.reviewStatus === "APPROVED").length, regenerate: items.filter((item) => item.reviewStatus === "REGENERATE").length }; }
