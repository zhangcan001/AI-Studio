import { useEffect, useMemo, useState } from "react";
import {
  cloneWorkflowBenchmark,
  createWorkflowBenchmark,
  deleteWorkflowBenchmark,
  getAsset,
  getWorkflowBenchmark,
  listPresets,
  listWorkflowBenchmarks,
  previewWorkflowBenchmark,
  queueWorkflowBenchmark,
  saveWorkflowBenchmarkQuality,
  setProductionReviewStatus,
  setWorkflowBenchmarkRecommendation,
  setWorkflowBenchmarkWinner,
} from "../../services/tauriClient";
import { toUserMessage } from "../../i18n/errorMessages";
import type { AssetView } from "../../types/asset";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import type { PresetView } from "../../types/preset";
import type {
  BenchmarkCandidateCompatibility,
  BenchmarkMediaType,
  BenchmarkSeedMode,
  WorkflowBenchmarkCandidatePreview,
  WorkflowBenchmarkCandidateRequest,
  WorkflowBenchmarkCandidateView,
  WorkflowBenchmarkCreateRequest,
  WorkflowBenchmarkQuality,
  WorkflowBenchmarkSummary,
  WorkflowBenchmarkView,
} from "../../types/benchmark";
import { ProductionAssetPreview } from "../studio/ProductionAssetPreview";
import {
  benchmarkAdmissionNotice,
  canRunBenchmarkDraft,
  previewForCandidatePosition,
} from "./workflowBenchmarkUi";

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  baseRecipe?: RecipeViewModel;
  baseValues: GenerationValues;
  baseReady: boolean;
  blockedReason?: string;
  onOpenTask?: (taskId: string) => void;
  onAdmissionChanged?: () => Promise<void>;
  onCreated?: (experiment: WorkflowBenchmarkView) => void;
}

interface CandidateDraft extends WorkflowBenchmarkCandidateRequest {
  key: string;
}

const compatibilityLabels: Record<BenchmarkCandidateCompatibility, string> = {
  COMPATIBLE: "兼容",
  PARTIAL: "部分兼容",
  INCOMPATIBLE: "不兼容",
};

function recipeKey(recipe: Pick<RecipeViewModel, "workflowVersionId" | "recipeId">): string {
  return `${recipe.workflowVersionId}:${recipe.recipeId}`;
}

function mediaRecipes(catalog: RecipeViewModel[], mediaType: BenchmarkMediaType): RecipeViewModel[] {
  const output = mediaType.toLowerCase() as "image" | "video";
  return catalog.filter((recipe) => recipe.outputTypes?.includes(output));
}

function recipeLabel(recipe: RecipeViewModel): string {
  return `${recipe.name}${recipe.recipeVersion ? ` · v${recipe.recipeVersion}` : ""}`;
}

function statusLabel(status: string): string {
  switch (status) {
    case "QUEUED": return "排队中";
    case "RUNNING": return "生成中";
    case "COMPLETED": return "已完成";
    case "PARTIAL": return "部分完成";
    case "CANCELLED": return "已取消";
    case "FAILED_TO_QUEUE": return "未入队";
    default: return "未知状态";
  }
}

function taskStatusLabel(status?: string): string {
  switch (status) {
    case "SUCCEEDED": return "已完成";
    case "FAILED": return "失败";
    case "CANCELLED": return "已取消";
    case "IN_PROGRESS": return "执行中";
    case "PENDING": return "等待中";
    default: return status ? "未知状态" : "等待中";
  }
}

function mediaTypeLabel(mediaType: BenchmarkMediaType): string {
  return mediaType === "VIDEO" ? "视频" : "图片";
}

function recommendationLabel(kind: string): string {
  return { FASTEST: "最快", MOST_STABLE: "最稳定", BEST_QUALITY: "最佳画质", BEST_BALANCE: "最佳平衡" }[kind] ?? "推荐";
}

function runtimeProfileLabel(profile?: string): string {
  if (!profile) return "—";
  if (profile === "QUALITY" || profile === "H3_QUALITY") return "高质量模式";
  if (profile === "FAST" || profile === "H3_FAST") return "快速模式";
  return profile;
}

function formatDuration(duration?: number): string {
  if (duration === undefined) return "—";
  if (duration < 1000) return `${duration} ms`;
  return `${(duration / 1000).toFixed(1)} s`;
}

function formatMetric(duration?: number): string {
  return duration === undefined ? "—" : formatDuration(duration);
}

function formatRate(rate: number): string {
  return `${Math.round(rate * 100)}%`;
}

function mediaTypeForRecipe(recipe?: RecipeViewModel): BenchmarkMediaType {
  return recipe?.outputTypes?.includes("video") ? "VIDEO" : "IMAGE";
}

function initialCandidates(catalog: RecipeViewModel[], baseRecipe: RecipeViewModel | undefined, mediaType: BenchmarkMediaType): CandidateDraft[] {
  const compatible = mediaRecipes(catalog, mediaType);
  const preferred = baseRecipe && mediaTypeForRecipe(baseRecipe) === mediaType ? baseRecipe : compatible[0];
  const alternative = compatible.find((recipe) => recipeKey(recipe) !== (preferred ? recipeKey(preferred) : ""));
  return [preferred, alternative]
    .filter((recipe): recipe is RecipeViewModel => Boolean(recipe))
    .map((recipe, index) => ({
      key: `${recipeKey(recipe)}-${index}`,
      workflowVersionId: recipe.workflowVersionId,
      recipeId: recipe.recipeId,
      label: `候选 ${index + 1}`,
    }));
}

function candidateRequests(candidates: CandidateDraft[]): WorkflowBenchmarkCandidateRequest[] {
  return candidates.map(({ key: _key, ...candidate }) => candidate);
}

function compatibilityClass(compatibility: string): string {
  return compatibility === "COMPATIBLE"
    ? "benchmark-compatibility benchmark-compatibility-good"
    : compatibility === "PARTIAL"
      ? "benchmark-compatibility benchmark-compatibility-partial"
      : "benchmark-compatibility benchmark-compatibility-bad";
}

function candidateTitle(candidate: WorkflowBenchmarkCandidatePreview, catalog: RecipeViewModel[]): string {
  const recipe = catalog.find((item) => item.workflowVersionId === candidate.workflowVersionId && item.recipeId === candidate.recipeId);
  return recipe ? recipeLabel(recipe) : `${candidate.workflowVersionId} · ${candidate.recipeId}`;
}

function reviewLabel(status?: string): string {
  switch (status) {
    case "STARRED": return "最佳";
    case "APPROVED": return "可用";
    case "REJECTED": return "不推荐";
    default: return "未审";
  }
}

export function WorkflowBenchmarkPanel({
  projectId,
  catalog,
  baseRecipe,
  baseValues,
  baseReady,
  blockedReason,
  onOpenTask,
  onAdmissionChanged,
  onCreated,
}: Props) {
  const [mediaType, setMediaType] = useState<BenchmarkMediaType>(() => mediaTypeForRecipe(baseRecipe));
  const [name, setName] = useState("工作流横向基准实验");
  const [candidates, setCandidates] = useState<CandidateDraft[]>(() => initialCandidates(catalog, baseRecipe, mediaType));
  const [seedMode, setSeedMode] = useState<BenchmarkSeedMode>("FIXED");
  const [repeatCount, setRepeatCount] = useState<1 | 3 | 5 | 10>(3);
  const [fixedSeed, setFixedSeed] = useState(() => {
    const seed = Object.values(baseValues).find((value) => value.type === "seed_fixed");
    return seed?.type === "seed_fixed" ? seed.value : "";
  });
  const [presets, setPresets] = useState<Record<string, PresetView[]>>({});
  const [preview, setPreview] = useState<WorkflowBenchmarkCandidatePreview[]>();
  const [history, setHistory] = useState<WorkflowBenchmarkSummary[]>([]);
  const [selected, setSelected] = useState<WorkflowBenchmarkView>();
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [previewAsset, setPreviewAsset] = useState<AssetView>();
  const [qualityDrafts, setQualityDrafts] = useState<Record<string, WorkflowBenchmarkQuality>>({});

  const availableRecipes = useMemo(() => mediaRecipes(catalog, mediaType), [catalog, mediaType]);
  useEffect(() => {
    setCandidates(initialCandidates(catalog, baseRecipe, mediaType));
    setPreview(undefined);
  }, [baseRecipe?.recipeId, baseRecipe?.workflowVersionId, catalog, mediaType]);

  useEffect(() => {
    let active = true;
    setLoadingHistory(true);
    void listWorkflowBenchmarks(projectId, 20)
      .then((items) => { if (active) setHistory(items); })
      .catch((loadError: unknown) => { if (active) setError(toUserMessage(loadError)); })
      .finally(() => { if (active) setLoadingHistory(false); });
    return () => { active = false; };
  }, [projectId]);

  useEffect(() => {
    let active = true;
    const keys = [...new Set(candidates.map((candidate) => recipeKey(candidate)))];
    void Promise.all(keys.map(async (key) => {
      const [workflowVersionId, recipeId] = key.split(":");
      try {
        return [key, await listPresets(projectId, workflowVersionId, recipeId)] as const;
      } catch {
        return [key, []] as const;
      }
    })).then((entries) => {
      if (!active) return;
      setPresets((current) => ({ ...current, ...Object.fromEntries(entries) }));
    });
    return () => { active = false; };
  }, [candidates, projectId]);

  function updateCandidate(key: string, patch: Partial<CandidateDraft>) {
    setCandidates((current) => current.map((candidate) => candidate.key === key ? { ...candidate, ...patch } : candidate));
    setPreview(undefined);
    setError(undefined);
  }

  function selectRecipe(key: string, value: string) {
    const [workflowVersionId, recipeId] = value.split(":");
    updateCandidate(key, { workflowVersionId, recipeId, presetId: undefined });
  }

  function addCandidate() {
    if (candidates.length >= 8 || !availableRecipes.length) return;
    const nextRecipe = availableRecipes.find((recipe) => !candidates.some((candidate) => recipeKey(candidate) === recipeKey(recipe))) ?? availableRecipes[0];
    setCandidates((current) => [...current, {
      key: `${recipeKey(nextRecipe)}-${Date.now()}`,
      workflowVersionId: nextRecipe.workflowVersionId,
      recipeId: nextRecipe.recipeId,
      label: `候选 ${current.length + 1}`,
    }]);
    setPreview(undefined);
  }

  function moveCandidate(index: number, offset: -1 | 1) {
    const target = index + offset;
    if (target < 0 || target >= candidates.length) return;
    setCandidates((current) => {
      const next = [...current];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
    setPreview(undefined);
  }

  function request(): WorkflowBenchmarkCreateRequest {
    return {
      projectId,
      name,
      mediaType,
      baseValues,
      candidates: candidateRequests(candidates),
      seedMode,
      fixedSeed: fixedSeed.trim() || undefined,
      repeatCount,
      autoStart: false,
    };
  }

  async function previewCandidates() {
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const result = await previewWorkflowBenchmark(request());
      setPreview(result.candidates);
      if (result.candidates.some((candidate) => candidate.compatibility === "INCOMPATIBLE")) {
        setError("存在不兼容候选，请修正后再创建基准实验。");
      } else {
        setNotice("候选已按语义输入校验，冻结值可在下方复核。不会自动启动。 ");
      }
    } catch (previewError: unknown) {
      setPreview(undefined);
      setError(toUserMessage(previewError));
    } finally {
      setBusy(false);
    }
  }

  async function create(autoStart: boolean) {
    if (candidates.length < 2 || candidates.length > 8 || !name.trim()) {
      setError("基准实验需要名称和 2–8 个候选。");
      return;
    }
    if (!baseReady) {
      setError(`请先完成基础输入：${blockedReason ?? "当前基础草稿尚未通过校验。"}`);
      return;
    }
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const checked = await previewWorkflowBenchmark(request());
      setPreview(checked.candidates);
      if (checked.candidates.some((candidate) => candidate.compatibility === "INCOMPATIBLE")) {
        setError("存在不兼容候选，未创建队列。");
        return;
      }
      const created = await createWorkflowBenchmark({ ...request(), autoStart });
      setSelected(created);
      setHistory((current) => [created.summary, ...current.filter((item) => item.id !== created.id)]);
      onCreated?.(created);
      await onAdmissionChanged?.();
      setNotice(benchmarkAdmissionNotice(created.status, autoStart));
    } catch (createError: unknown) {
      setError(toUserMessage(createError));
    } finally {
      setBusy(false);
    }
  }

  async function openHistory(id: string) {
    setBusy(true);
    setError(undefined);
    try {
      setSelected(await getWorkflowBenchmark(projectId, id));
    } catch (loadError: unknown) {
      setError(toUserMessage(loadError));
    } finally {
      setBusy(false);
    }
  }

  async function refreshSelected() {
    if (!selected) return;
    await openHistory(selected.id);
  }

  async function markWinner(candidateId?: string) {
    if (!selected) return;
    setBusy(true);
    try {
      setSelected(await setWorkflowBenchmarkWinner(projectId, selected.id, candidateId));
      setHistory((current) => current.map((item) => item.id === selected.id ? { ...item, winnerCandidateId: candidateId } : item));
    } catch (winnerError: unknown) {
      setError(toUserMessage(winnerError));
    } finally {
      setBusy(false);
    }
  }

  function qualityFor(candidate: WorkflowBenchmarkCandidateView): WorkflowBenchmarkQuality {
    return qualityDrafts[candidate.id] ?? candidate.quality ?? {};
  }

  function updateQuality(candidateId: string, patch: Partial<WorkflowBenchmarkQuality>) {
    setQualityDrafts((current) => ({ ...current, [candidateId]: { ...current[candidateId], ...patch } }));
  }

  async function saveQuality(candidate: WorkflowBenchmarkCandidateView) {
    if (!selected) return;
    setBusy(true);
    try {
      const updated = await saveWorkflowBenchmarkQuality(projectId, selected.id, candidate.id, qualityFor(candidate));
      setSelected(updated);
      setQualityDrafts((current) => ({ ...current, [candidate.id]: updated.candidates.find((item) => item.id === candidate.id)?.quality ?? {} }));
      setNotice("人工质量评分已保存，不影响性能统计。 ");
    } catch (qualityError: unknown) {
      setError(toUserMessage(qualityError));
    } finally {
      setBusy(false);
    }
  }

  async function markRecommendation(kind: string, candidateId?: string) {
    if (!selected || !candidateId) return;
    setBusy(true);
    try {
      const updated = await setWorkflowBenchmarkRecommendation(projectId, selected.id, kind);
      setSelected(updated);
      setNotice(`已记录 ${kind} 推荐：${updated.candidates.find((candidate) => candidate.id === candidateId)?.label ?? "候选"}。`);
    } catch (recommendationError: unknown) {
      setError(toUserMessage(recommendationError));
    } finally {
      setBusy(false);
    }
  }

  async function markReview(candidate: WorkflowBenchmarkCandidateView, status: "STARRED" | "APPROVED" | "REJECTED") {
    if (!selected?.productionBatchId || !candidate.productionBatchItemId) return;
    setBusy(true);
    try {
      await setProductionReviewStatus({ projectId, batchId: selected.productionBatchId, itemId: candidate.productionBatchItemId, status });
      await refreshSelected();
    } catch (reviewError: unknown) {
      setError(toUserMessage(reviewError));
    } finally {
      setBusy(false);
    }
  }

  async function openAsset(assetId: string) {
    setBusy(true);
    try {
      setPreviewAsset(await getAsset(projectId, assetId));
    } catch (assetError: unknown) {
      setError(toUserMessage(assetError));
    } finally {
      setBusy(false);
    }
  }

  async function cloneSelected() {
    if (!selected) return;
    setBusy(true);
    try {
      const cloned = await cloneWorkflowBenchmark(projectId, selected.id);
      setSelected(cloned);
      setHistory((current) => [cloned.summary, ...current]);
      setNotice("基准实验已克隆；任务、结果、审片和胜者不会被复制。");
    } catch (cloneError: unknown) {
      setError(toUserMessage(cloneError));
    } finally {
      setBusy(false);
    }
  }

  async function queueSelected(autoStart: boolean) {
    if (!selected || !canRunBenchmarkDraft(selected.status, selected.productionBatchId)) return;
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const queued = await queueWorkflowBenchmark(projectId, selected.id, autoStart);
      setSelected(queued);
      setHistory((current) => [queued.summary, ...current.filter((item) => item.id !== queued.id)]);
      await onAdmissionChanged?.();
      setNotice(benchmarkAdmissionNotice(queued.status, autoStart));
    } catch (queueError: unknown) {
      setError(toUserMessage(queueError));
    } finally {
      setBusy(false);
    }
  }

  async function deleteSelected() {
    if (!selected || !window.confirm("只删除基准实验历史元数据，不删除任务、批次、资产或审片记录。继续？")) return;
    setBusy(true);
    try {
      await deleteWorkflowBenchmark(projectId, selected.id);
      setHistory((current) => current.filter((item) => item.id !== selected.id));
      setSelected(undefined);
      setNotice("基准实验元数据已删除；普通生产历史保持不变。");
    } catch (deleteError: unknown) {
      setError(toUserMessage(deleteError));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="workflow-benchmark-panel" aria-label="工作流基准实验室">
      <div className="workflow-benchmark-heading">
        <div>
          <span className="section-label">工作流基准实验室</span>
          <h3>横向比较工作流 / 配方 / 预设</h3>
          <p>相同基础输入、素材和 Seed，候选只通过现有生产队列串行执行；结果由人审，不自动选胜者。</p>
        </div>
          <span className="workflow-benchmark-badge">v2 · 2–8 候选 · 串行重复</span>
      </div>

      <div className="workflow-benchmark-create-grid">
        <label>
          <span>实验名称</span>
          <input value={name} maxLength={120} onChange={(event) => setName(event.target.value)} placeholder="例如：H3 低成本参数对比" />
        </label>
        <label>
          <span>媒体类型</span>
          <select value={mediaType} onChange={(event) => setMediaType(event.target.value as BenchmarkMediaType)}>
            <option value="IMAGE">图片</option>
            <option value="VIDEO">视频</option>
          </select>
        </label>
        <label>
          <span>Seed 策略</span>
          <select value={seedMode} onChange={(event) => setSeedMode(event.target.value as BenchmarkSeedMode)}>
            <option value="FIXED">固定 Seed · 推荐</option>
            <option value="EXPLORATION">探索模式 · 冻结随机 Seed</option>
          </select>
        </label>
        {seedMode === "FIXED" && (
          <label>
            <span>固定 Seed <small>可选</small></span>
            <input value={fixedSeed} inputMode="numeric" onChange={(event) => setFixedSeed(event.target.value.replace(/[^0-9]/g, ""))} placeholder="留空则沿用基础输入" />
          </label>
        )}
        <label>
          <span>每候选运行次数</span>
          <select value={repeatCount} onChange={(event) => setRepeatCount(Number(event.target.value) as 1 | 3 | 5 | 10)}>
            {[1, 3, 5, 10].map((count) => <option value={count} key={count}>{count} 次 · 严格串行</option>)}
          </select>
        </label>
      </div>

      <div className="workflow-benchmark-candidate-toolbar">
        <div>
          <strong>候选冻结</strong>
          <small>按语义键对齐输入；预设只记录来源，数值会冻结到候选。</small>
        </div>
        <button type="button" className="quiet-button" onClick={addCandidate} disabled={busy || candidates.length >= 8 || !availableRecipes.length}>添加候选</button>
      </div>
      <div className="workflow-benchmark-candidates">
        {candidates.map((candidate, index) => {
          const key = recipeKey(candidate);
          const candidatePresets = presets[key] ?? [];
          const candidatePreview = previewForCandidatePosition(preview, index);
          return (
            <article className="workflow-benchmark-candidate" key={candidate.key}>
              <div className="workflow-benchmark-candidate-index">#{index + 1}</div>
              <div className="workflow-benchmark-candidate-fields">
                <label>
                  <span>工作流 / 配方</span>
                  <select value={key} onChange={(event) => selectRecipe(candidate.key, event.target.value)} disabled={!availableRecipes.length}>
                    {availableRecipes.map((recipe) => <option key={recipeKey(recipe)} value={recipeKey(recipe)}>{recipeLabel(recipe)}</option>)}
                  </select>
                </label>
                <label>
                  <span>预设 <small>可选</small></span>
                  <select value={candidate.presetId ?? ""} onChange={(event) => updateCandidate(candidate.key, { presetId: event.target.value || undefined })}>
                    <option value="">不使用预设</option>
                    {candidatePresets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}</option>)}
                  </select>
                </label>
                <label>
                  <span>候选标签</span>
                  <input value={candidate.label ?? ""} maxLength={80} onChange={(event) => updateCandidate(candidate.key, { label: event.target.value })} />
                </label>
              </div>
              <div className="workflow-benchmark-candidate-actions">
                <button type="button" className="quiet-button" onClick={() => moveCandidate(index, -1)} disabled={index === 0}>上移</button>
                <button type="button" className="quiet-button" onClick={() => moveCandidate(index, 1)} disabled={index === candidates.length - 1}>下移</button>
                <button type="button" className="quiet-button" onClick={() => { setCandidates((current) => current.filter((item) => item.key !== candidate.key)); setPreview(undefined); }} disabled={candidates.length <= 2}>移除</button>
              </div>
              {candidatePreview && (
                <div className="workflow-benchmark-preview-row">
                  <span className={compatibilityClass(candidatePreview.compatibility)}>{compatibilityLabels[candidatePreview.compatibility]}</span>
                  <span>{candidatePreview.assetIds.length} 个素材 ID 已冻结</span>
                  {candidatePreview.compatibilityReasons.map((reason) => <small key={reason}>{reason}</small>)}
                </div>
              )}
            </article>
          );
        })}
        {!candidates.length && <p className="disabled-note">当前媒体类型没有可用配方。</p>}
      </div>

      <div className="workflow-benchmark-create-actions">
        <button type="button" className="quiet-button" onClick={() => void previewCandidates()} disabled={busy || candidates.length < 2}>预览兼容性与冻结值</button>
        <button type="button" onClick={() => void create(false)} disabled={busy || candidates.length < 2 || !baseReady}>创建基准实验（不启动）</button>
        <button type="button" onClick={() => void create(true)} disabled={busy || candidates.length < 2 || !baseReady}>创建并开始实验</button>
      </div>
      {!baseReady && <p className="workflow-benchmark-blocked" role="status">基础输入尚未就绪：{blockedReason ?? "请先完成当前创作参数。"}</p>}
      {error && <p className="error-message" role="alert">{error}</p>}
      {notice && <p className="studio-notice" role="status">{notice}</p>}

      <div className="workflow-benchmark-history-heading">
        <div>
          <span className="section-label">历史实验</span>
          <strong>可重启恢复的基准实验结果</strong>
        </div>
        {loadingHistory && <small>加载中…</small>}
      </div>
      <div className="workflow-benchmark-history-list">
        {history.map((item) => (
          <button type="button" className={`workflow-benchmark-history-row${selected?.id === item.id ? " workflow-benchmark-history-row-active" : ""}`} key={item.id} onClick={() => void openHistory(item.id)}>
            <span><strong>{item.name}</strong><small>{mediaTypeLabel(item.mediaType)} · {statusLabel(item.status)}</small></span>
            <span>{item.succeededCount}/{item.candidateCount} 候选 · {item.repeatCount} 次/候选</span>
            <span>{item.winnerCandidateId ? "已指定胜者" : "未指定胜者"}</span>
          </button>
        ))}
        {!history.length && !loadingHistory && <p className="disabled-note">还没有历史基准实验。</p>}
      </div>

      {selected && (
        <section className="workflow-benchmark-result" aria-label="基准实验结果">
          <div className="workflow-benchmark-result-heading">
            <div>
              <span className="section-label">结果审阅</span>
              <h4>{selected.name}</h4>
              <p>{mediaTypeLabel(selected.mediaType)} · {statusLabel(selected.status)} · {selected.summary.succeededCount}/{selected.summary.candidateCount} 候选完成 · {selected.summary.repeatCount} 次/候选 · 最快：{selected.summary.fastestCandidateId ? formatDuration(selected.summary.fastestDurationMs) : "—"}</p>
            </div>
            <div className="workflow-benchmark-result-actions">
              <button type="button" className="quiet-button" onClick={() => void refreshSelected()} disabled={busy}>刷新</button>
              <button type="button" className="quiet-button" onClick={() => void cloneSelected()} disabled={busy}>克隆</button>
              {canRunBenchmarkDraft(selected.status, selected.productionBatchId) && (
                <>
                  <button type="button" className="quiet-button" onClick={() => void queueSelected(false)} disabled={busy}>加入生产队列</button>
                  <button type="button" onClick={() => void queueSelected(true)} disabled={busy}>运行此克隆</button>
                </>
              )}
              <button type="button" className="quiet-button" onClick={() => void deleteSelected()} disabled={busy}>删除历史</button>
            </div>
          </div>
          <div className="workflow-benchmark-comparison" role="status">
            <strong>可比性与推荐</strong>
            <span>{selected.comparison.directlyComparable ? `可直接比较 · Seed ${selected.summary.seedStrategy}` : selected.comparison.reason ?? "暂不可比较"}</span>
            {selected.comparison.recommendations.map((recommendation) => {
              const candidate = selected.candidates.find((item) => item.id === recommendation.candidateId);
              return (
                <div key={recommendation.kind} className="workflow-benchmark-recommendation">
                  <span><b>{recommendationLabel(recommendation.kind)}</b> · {candidate?.label ?? "暂无"} · {recommendation.rationale}</span>
                  {candidate && <button type="button" className="quiet-button" onClick={() => void markRecommendation(recommendation.kind, candidate.id)} disabled={busy}>{selected.summary.recommendationType === recommendation.kind ? "已记录" : "记录推荐"}</button>}
                </div>
              );
            })}
          </div>
          <div className="workflow-benchmark-result-list">
            {selected.candidates.map((candidate) => {
              const isWinner = selected.winnerCandidateId === candidate.id;
              const isFastest = selected.summary.fastestCandidateId === candidate.id;
              return (
                <article className={`workflow-benchmark-result-card${isWinner ? " workflow-benchmark-result-card-winner" : ""}`} key={candidate.id}>
                  <div className="workflow-benchmark-result-card-main">
                    <div className="workflow-benchmark-result-card-title"><strong>#{candidate.position + 1} · {candidate.label}</strong><span className={compatibilityClass(candidate.compatibility)}>{compatibilityLabels[candidate.compatibility]}</span></div>
                    <small>{candidateTitle(candidate, catalog)} · {taskStatusLabel(candidate.taskStatus)} · {candidate.aggregate.runsSuccess}/{candidate.aggregate.runsTotal} 次运行 · 成功率 {formatRate(candidate.aggregate.successRate)} · 中位总耗时 {formatMetric(candidate.aggregate.totalMs.median)} · P95 {formatMetric(candidate.aggregate.totalMs.p95)} · Comfy 中位 {formatMetric(candidate.aggregate.comfyExecutionMs.median)} · 输出 {candidate.aggregate.outputSizeMean ?? "—"} 字节 · {runtimeProfileLabel(candidate.runtimeProfile ?? candidate.telemetry?.runtimeProfile)} · 审片：{reviewLabel(candidate.reviewStatus)}</small>
                    <small>工作流 SHA {candidate.workflowSha256 ?? "—"} · 配方 SHA {candidate.recipeSha256 ?? "—"} · 编译后 SHA {candidate.runs.find((run) => run.compiledWorkflowSha256)?.compiledWorkflowSha256 ?? "—"}</small>
                    {isWinner && <span className="workflow-benchmark-winner-mark">显式胜者</span>}
                    {isFastest && <span className="workflow-benchmark-fastest-mark">最快完成</span>}
                    {candidate.outputAssetIds.length > 0 && (
                      <div className="workflow-benchmark-output-assets">
                        {candidate.outputAssetIds.map((assetId) => <button type="button" className="quiet-button" key={assetId} onClick={() => void openAsset(assetId)}>查看生成结果</button>)}
                      </div>
                    )}
                  </div>
                  <div className="workflow-benchmark-result-card-actions">
                    {candidate.taskId && onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(candidate.taskId!)}>打开任务</button>}
                    <button type="button" className="quiet-button" onClick={() => void markReview(candidate, "STARRED")} disabled={!candidate.productionBatchItemId || busy}>标为最佳</button>
                    <button type="button" className="quiet-button" onClick={() => void markReview(candidate, "APPROVED")} disabled={!candidate.productionBatchItemId || busy}>可用</button>
                    <button type="button" className="quiet-button" onClick={() => void markReview(candidate, "REJECTED")} disabled={!candidate.productionBatchItemId || busy}>不推荐</button>
                    <button type="button" onClick={() => void markWinner(isWinner ? undefined : candidate.id)} disabled={busy}>{isWinner ? "取消胜者" : "指定胜者"}</button>
                  </div>
                  <div className="workflow-benchmark-quality-editor">
                    <span>人工质量（可选）</span>
                    <label>提示词 <select value={qualityFor(candidate).promptAdherence ?? ""} onChange={(event) => updateQuality(candidate.id, { promptAdherence: event.target.value ? Number(event.target.value) : undefined })}><option value="">—</option>{[1, 2, 3, 4, 5].map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
                    <label>画面 <select value={qualityFor(candidate).visualQuality ?? ""} onChange={(event) => updateQuality(candidate.id, { visualQuality: event.target.value ? Number(event.target.value) : undefined })}><option value="">—</option>{[1, 2, 3, 4, 5].map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
                    {selected.mediaType === "VIDEO" && <label>运动 <select value={qualityFor(candidate).motionQuality ?? ""} onChange={(event) => updateQuality(candidate.id, { motionQuality: event.target.value ? Number(event.target.value) : undefined })}><option value="">—</option>{[1, 2, 3, 4, 5].map((value) => <option key={value} value={value}>{value}</option>)}</select></label>}
                    <label>参考 <select value={qualityFor(candidate).referenceConsistency ?? ""} onChange={(event) => updateQuality(candidate.id, { referenceConsistency: event.target.value ? Number(event.target.value) : undefined })}><option value="">—</option>{[1, 2, 3, 4, 5].map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
                    <label>总体 <select value={qualityFor(candidate).overall ?? ""} onChange={(event) => updateQuality(candidate.id, { overall: event.target.value ? Number(event.target.value) : undefined })}><option value="">—</option>{[1, 2, 3, 4, 5].map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
                    <button type="button" className="quiet-button" onClick={() => void saveQuality(candidate)} disabled={busy}>保存评分</button>
                  </div>
                </article>
              );
            })}
          </div>
        </section>
      )}
      {previewAsset && <ProductionAssetPreview projectId={projectId} asset={previewAsset} onClose={() => setPreviewAsset(undefined)} onOpenTask={onOpenTask} />}
    </section>
  );
}
