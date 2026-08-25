import { useEffect, useMemo, useState } from "react";
import {
  applyPromptTemplate,
  bulkSetShotStageConfig,
  getEpisodeProductionPlan,
  listBatchWorkflowPresets,
  prepareEpisodeProduction,
  previewPromptTemplateBulk,
} from "../../services/tauriClient";
import { formatUiError } from "../../i18n/errorMessages";
import type {
  EpisodeProductionFilter,
  EpisodeProductionPanelProps,
  EpisodeProductionPlan,
  EpisodeProductionPrepareResult,
  EpisodeProductionSceneClassification,
  EpisodeProductionScenePlan,
  EpisodeProductionStage,
} from "../../types/episodeProduction";
import type { BatchWorkflowPreset } from "../../types/sceneProduction";
import { sceneProductionStageLabel, sceneProductionStagePreset } from "../../types/sceneProduction";
import { analyzePromptTemplateText, customPromptVariableNames } from "../prompts/promptTemplateState";
import { orderedEpisodes, orderedScenes, orderedSeries } from "./productionStructureState";
import "./EpisodeProductionPanel.css";

type BusyAction = "plan" | "preset-apply" | "prompt-preview" | "prompt-apply" | "prepare";
type EpisodeError = { code: string; message: string; technicalMessage?: string };

const STAGES: EpisodeProductionStage[] = ["image", "video"];
const FILTERS: EpisodeProductionFilter[] = ["all", "ready", "prepared", "blocked", "done"];

export function EpisodeProductionPanel({
  projectId,
  tree,
  shots,
  promptEntries = [],
  referenceAnchors = [],
  initialPresets = [],
  initialPlan,
  onRefresh,
  onNotice,
  onError,
  onOpenProductionQueue,
  onNavigateToScene,
}: EpisodeProductionPanelProps) {
  const episodeOptions = useMemo(() => productionEpisodeOptions(tree), [tree]);
  const [episodeId, setEpisodeId] = useState(initialPlan?.episodeId ?? episodeOptions[0]?.value ?? "");
  const [stage, setStage] = useState<EpisodeProductionStage>(initialPlan?.stage ?? "image");
  const [plan, setPlan] = useState<EpisodeProductionPlan | undefined>(initialPlan);
  const [selectedSceneIds, setSelectedSceneIds] = useState<string[]>([]);
  const [filter, setFilter] = useState<EpisodeProductionFilter>("all");
  const [allowPartial, setAllowPartial] = useState(false);
  const [busyAction, setBusyAction] = useState<BusyAction>();
  const [error, setError] = useState<EpisodeError>();
  const [notice, setNotice] = useState<string>();
  const [result, setResult] = useState<EpisodeProductionPrepareResult>();
  const [presets, setPresets] = useState<BatchWorkflowPreset[]>(initialPresets);
  const [selectedPresetId, setSelectedPresetId] = useState(initialPresets[0]?.id ?? "");
  const [promptEntryId, setPromptEntryId] = useState("");
  const [promptVersionId, setPromptVersionId] = useState("");
  const [anchorIds, setAnchorIds] = useState<string[]>([]);
  const [customValues, setCustomValues] = useState<Record<string, string>>({});
  const [promptPreview, setPromptPreview] = useState<{ total: number; valid: number; invalid: number }>();

  const selectedPreset = presets.find((item) => item.id === selectedPresetId);
  const selectedPromptEntry = promptEntries.find((item) => item.id === promptEntryId);
  const selectedPromptVersion = selectedPromptEntry?.versions.find((item) => item.id === promptVersionId)
    ?? selectedPromptEntry?.versions[selectedPromptEntry.versions.length - 1];
  const promptCustomNames = useMemo(
    () => selectedPromptVersion ? customPromptVariableNames(analyzePromptTemplateText(selectedPromptVersion.text).customVariables) : [],
    [selectedPromptVersion],
  );
  const isBusy = episodeProductionActionDisabled(busyAction);
  const scenePlans = plan?.scenes ?? [];
  const selectedPlans = scenePlans.filter((scene) => selectedSceneIds.includes(scene.sceneId));
  const selectedShotIds = useMemo(
    () => episodeSceneShotIds(tree, shots, selectedSceneIds),
    [tree, shots, selectedSceneIds],
  );
  const visibleScenes = useMemo(
    () => scenePlans.filter((scene) => matchesEpisodeFilter(scene, filter)),
    [scenePlans, filter],
  );
  const currentEpisode = episodeOptions.find((item) => item.value === episodeId);

  useEffect(() => {
    if (episodeId && episodeOptions.some((item) => item.value === episodeId)) return;
    setEpisodeId(episodeOptions[0]?.value ?? "");
  }, [episodeId, episodeOptions]);

  useEffect(() => {
    if (!episodeId || (initialPlan?.episodeId === episodeId && initialPlan.stage === stage)) return;
    let active = true;
    setBusyAction("plan");
    setError(undefined);
    void getEpisodeProductionPlan({ projectId, episodeId, stage })
      .then((next) => { if (active) setPlan(next); })
      .catch((value: unknown) => { if (active) setError(toEpisodeError(value, "EPISODE_PRODUCTION_PLAN_FAILED")); })
      .finally(() => { if (active) setBusyAction(undefined); });
    return () => { active = false; };
  }, [episodeId, initialPlan, projectId, stage]);

  useEffect(() => {
    setSelectedSceneIds((current) => current.filter((sceneId) => scenePlans.some((scene) => scene.sceneId === sceneId)));
  }, [scenePlans]);

  useEffect(() => {
    if (initialPresets.length) return;
    let active = true;
    void listBatchWorkflowPresets()
      .then((next) => {
        if (!active) return;
        setPresets(next);
        setSelectedPresetId((current) => current && next.some((item) => item.id === current) ? current : next[0]?.id ?? "");
      })
      .catch((value: unknown) => { if (active) setError(toEpisodeError(value, "BATCH_WORKFLOW_PRESETS_LOAD_FAILED")); });
    return () => { active = false; };
  }, [initialPresets.length]);

  useEffect(() => {
    const entry = promptEntries[0];
    setPromptEntryId((current) => current && promptEntries.some((item) => item.id === current) ? current : entry?.id ?? "");
  }, [promptEntries]);

  useEffect(() => {
    const latest = selectedPromptEntry?.versions[selectedPromptEntry.versions.length - 1];
    setPromptVersionId((current) => current && selectedPromptEntry?.versions.some((item) => item.id === current) ? current : latest?.id ?? "");
  }, [selectedPromptEntry]);

  useEffect(() => {
    setCustomValues((current) => Object.fromEntries(promptCustomNames.map((name) => [name, current[name] ?? ""])));
    setPromptPreview(undefined);
  }, [promptCustomNames]);

  function clearFeedback() {
    setError(undefined);
    setNotice(undefined);
    onError?.("");
  }

  async function runAction(action: BusyAction, callback: () => Promise<void>) {
    if (busyAction) return;
    setBusyAction(action);
    clearFeedback();
    try {
      await callback();
    } catch (value: unknown) {
      const nextError = toEpisodeError(value, "EPISODE_PRODUCTION_ERROR");
      setError(nextError);
      onError?.(nextError.message);
    } finally {
      setBusyAction(undefined);
    }
  }

  async function refreshPlan() {
    if (!episodeId) return;
    await runAction("plan", async () => {
      const next = await getEpisodeProductionPlan({ projectId, episodeId, stage });
      setPlan(next);
    });
  }

  async function applyPresetForStage(targetStage: EpisodeProductionStage) {
    const stagePreset = sceneProductionStagePreset(selectedPreset, targetStage);
    if (!stagePreset || !selectedPreset || !selectedPlans.length) return;
    if (selectedShotIds.length > 500) {
      setError({ code: "EPISODE_BULK_SCOPE_TOO_LARGE", message: "所选场景超过 500 个镜头，未自动拆分请求。" });
      return;
    }
    if (!window.confirm(episodePresetConfirmation(targetStage, selectedPlans.length, selectedShotIds.length))) return;
    await runAction("preset-apply", async () => {
      await bulkSetShotStageConfig({
        projectId,
        stage: targetStage,
        shotIds: selectedShotIds,
        workflowVersionId: stagePreset.workflowVersionId,
        recipeId: stagePreset.recipeId,
        values: stagePreset.values,
      });
      setNotice(`已将“${selectedPreset.name}”应用到所选 ${selectedPlans.length} 个场景、${selectedShotIds.length} 个镜头。引用素材和已选媒体未改变。`);
      onNotice?.("集场景预设已应用。");
      await refreshAfterMutation();
    });
  }

  async function previewPrompt() {
    if (!selectedPromptEntry || !selectedPromptVersion || !selectedShotIds.length) return;
    await runAction("prompt-preview", async () => {
      const preview = await previewPromptTemplateBulk({
        projectId,
        promptEntryId: selectedPromptEntry.id,
        promptVersionId: selectedPromptVersion.id,
        shotIds: selectedShotIds,
        contextAnchorIds: anchorIds,
        customValues,
        previewLimit: 20,
      });
      setPromptPreview({ total: preview.total, valid: preview.valid, invalid: preview.invalid });
    });
  }

  async function applyPrompt() {
    if (!selectedPromptEntry || !selectedPromptVersion || !selectedShotIds.length || !promptPreview || promptPreview.invalid > 0) return;
    await runAction("prompt-apply", async () => {
      await applyPromptTemplate({
        projectId,
        promptEntryId: selectedPromptEntry.id,
        promptVersionId: selectedPromptVersion.id,
        stage,
        shotIds: selectedShotIds,
        contextAnchorIds: anchorIds,
        customValues,
      });
      setPromptPreview(undefined);
      setNotice(`已将提示词模板应用到所选 ${selectedPlans.length} 个场景、${selectedShotIds.length} 个镜头；每个镜头保留自己的场景上下文。`);
      await refreshAfterMutation();
    });
  }

  async function prepare() {
    if (!plan || !episodeId || !selectedPlans.length || !selectedShotIds.length) return;
    if (!allowPartial && selectedPlans.some((scene) => scene.blocked > 0)) {
      setResult(blockedPrepareResult(projectId, episodeId, stage, selectedPlans));
      setNotice("严格模式未创建批次：所选场景存在阻塞项。");
      return;
    }
    if (!window.confirm(episodePrepareConfirmation(plan.episodeName, stage, selectedPlans.length, selectedShotIds.length))) return;
    await runAction("prepare", async () => {
      try {
        const next = await prepareEpisodeProduction({ projectId, episodeId, stage, sceneIds: selectedSceneIds, allowPartial });
        setResult(next);
        setNotice(episodePrepareNotice(next));
        await refreshAfterMutation();
      } catch (value: unknown) {
        const nextError = toEpisodeError(value, "EPISODE_PRODUCTION_ERROR");
        if (nextError.code === "EPISODE_PRODUCTION_BLOCKED") {
          setResult(blockedPrepareResult(projectId, episodeId, stage, selectedPlans));
          setNotice("严格模式未创建批次：所选场景存在阻塞项。");
          return;
        }
        if (nextError.code === "EPISODE_PRODUCTION_PARTIAL") {
          const partial = parseEpisodePrepareErrorResult(value);
          if (partial) {
            setResult({ ...partial, status: "PARTIAL" });
            setNotice(episodePrepareNotice({ ...partial, status: "PARTIAL" }));
            await refreshAfterMutation();
            return;
          }
        }
        throw value;
      }
    });
  }

  async function refreshAfterMutation() {
    await onRefresh?.();
    const next = await getEpisodeProductionPlan({ projectId, episodeId, stage });
    setPlan(next);
  }

  function selectAll(mode: "ready" | "blocked" | "clear") {
    if (mode === "clear") {
      setSelectedSceneIds([]);
      return;
    }
    const next = scenePlans
      .filter((scene) => mode === "ready" ? scene.canPrepare : scene.blocked > 0)
      .map((scene) => scene.sceneId);
    setSelectedSceneIds(next);
  }

  function toggleScene(sceneId: string) {
    setSelectedSceneIds((current) => current.includes(sceneId) ? current.filter((id) => id !== sceneId) : [...current, sceneId]);
  }

  function selectStage(nextStage: EpisodeProductionStage) {
    setStage(nextStage);
    setResult(undefined);
    setPromptPreview(undefined);
  }

  if (!episodeOptions.length) {
    return <section className="episode-production-panel" aria-label="集生产规划"><div className="episode-production-empty"><strong>暂无可生产集</strong><span>请先在现有内容结构中创建集和场景。</span></div></section>;
  }

  return (
    <section className="episode-production-panel" aria-label="集生产规划" aria-busy={isBusy}>
      <div className="episode-production-header">
        <div><span className="section-label">集生产规划</span><h3>集生产规划</h3><p>一次检查多个场景，准备后仍由你按现有生产队列顺序启动。</p></div>
        <span className="episode-production-safety">不会自动启动 GPU · 不跨过人工审核</span>
      </div>

      <div className="episode-production-toolbar">
        <label><span>集</span><select value={episodeId} onChange={(event) => { setEpisodeId(event.target.value); setPlan(undefined); setSelectedSceneIds([]); }} disabled={isBusy} aria-label="集选择">{episodeOptions.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}</select></label>
        <div className="episode-production-stage-tabs" role="tablist" aria-label="生产阶段">{STAGES.map((nextStage) => <button key={nextStage} type="button" className={stage === nextStage ? "active" : ""} role="tab" aria-selected={stage === nextStage} onClick={() => selectStage(nextStage)} disabled={isBusy}>{sceneProductionStageLabel(nextStage)}</button>)}</div>
        <button type="button" className="quiet-button" onClick={() => void refreshPlan()} disabled={isBusy || !episodeId}>{busyAction === "plan" ? "检查中…" : "重新检查计划"}</button>
      </div>

      {plan ? <>
        <div className="episode-production-summary" aria-label="集汇总">
          <div><span>系列</span><strong>{plan.seriesName || currentEpisode?.seriesName || "—"}</strong></div>
          <div><span>集</span><strong>第 {String(plan.episodeOrdinal + 1).padStart(2, "0")} 集 · {plan.episodeName}</strong></div>
          <div><span>场景</span><strong>{plan.sceneTotal}</strong></div>
          <div><span>镜头</span><strong>{plan.shotTotal}</strong></div>
          <div><span>已完成</span><strong>{plan.done}</strong></div>
          <div><span>已准备</span><strong>{plan.prepared}</strong></div>
          <div><span>可生产</span><strong>{plan.eligible}</strong></div>
          <div><span>已阻塞</span><strong>{plan.blocked}</strong></div>
        </div>

        <section className="episode-production-card" aria-label="场景选择">
          <div className="episode-production-card-heading"><div><span className="section-label">场景范围</span><h4>场景总览</h4></div><span>{selectedSceneIds.length} 个已选 / {selectedShotIds.length} 个镜头</span></div>
          <div className="episode-production-selection-actions"><div className="episode-production-filter-tabs" role="group" aria-label="场景筛选">{FILTERS.map((value) => <button key={value} type="button" className={filter === value ? "active" : ""} onClick={() => setFilter(value)} disabled={isBusy}>{episodeFilterLabel(value)}</button>)}</div><div className="episode-production-select-actions"><button type="button" className="quiet-button" onClick={() => selectAll("ready")} disabled={isBusy}>全选可准备</button><button type="button" className="quiet-button" onClick={() => selectAll("blocked")} disabled={isBusy}>全选有阻塞</button><button type="button" className="quiet-button" onClick={() => selectAll("clear")} disabled={isBusy}>清空</button></div></div>
          <div className="episode-production-table-wrap"><table><thead><tr><th>选择</th><th>场景</th><th>镜头</th><th>已完成</th><th>已准备</th><th>可生产</th><th>已阻塞</th><th>状态</th><th /></tr></thead><tbody>{visibleScenes.map((scene) => <tr key={scene.sceneId}><td><input type="checkbox" checked={selectedSceneIds.includes(scene.sceneId)} onChange={() => toggleScene(scene.sceneId)} disabled={isBusy} aria-label={`选择场景 ${scene.sceneName}`} /></td><td><strong>{String(scene.sceneOrdinal + 1).padStart(2, "0")} · {scene.sceneName}</strong><small>{scene.sceneId}</small>{scene.blockingReasons.length > 0 && <small className="episode-production-blocker-reason">{scene.blockingReasons.join("；")}</small>}</td><td>{scene.total}</td><td>{scene.done}</td><td>{scene.prepared}</td><td>{scene.eligible}</td><td>{scene.blocked}</td><td><span className={`episode-production-status episode-production-status-${scene.classification.toLowerCase()}`}>{episodeSceneClassificationLabel(scene.classification)}</span></td><td>{scene.blocked > 0 && <button type="button" className="quiet-button" onClick={() => onNavigateToScene?.(scene.sceneId)} disabled={isBusy}>查看场景</button>}</td></tr>)}</tbody></table></div>
          {!visibleScenes.length && <div className="episode-production-empty"><strong>没有匹配的场景</strong><span>请切换筛选条件。</span></div>}
        </section>

        <div className="episode-production-grid">
          <section className="episode-production-card" aria-label="集批量预设">
            <div className="episode-production-card-heading"><div><span className="section-label">预设</span><h4>批量应用预设</h4></div><span>{presets.length} / 30</span></div>
            <label><span>批量工作流预设</span><select value={selectedPresetId} onChange={(event) => setSelectedPresetId(event.target.value)} disabled={isBusy}><option value="">选择预设</option>{presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}{!preset.available ? " · 不可用" : ""}</option>)}</select></label>
            <div className="episode-production-actions"><button type="button" onClick={() => { setStage("image"); void applyPresetForStage("image"); }} disabled={isBusy || !selectedPreset?.image || !selectedSceneIds.length}>{isBusy && busyAction === "preset-apply" ? "应用中…" : "应用图片预设到所选场景"}</button><button type="button" className="quiet-button" onClick={() => { setStage("video"); void applyPresetForStage("video"); }} disabled={isBusy || !selectedPreset?.video || !selectedSceneIds.length}>应用视频预设到所选场景</button></div>
            <small className="episode-production-hint">最多一次应用 500 个镜头；不会改变参考素材、已确认图片、已确认视频、锚点或场景归属。</small>
          </section>

          <section className="episode-production-card" aria-label="集提示词批量应用">
            <div className="episode-production-card-heading"><div><span className="section-label">提示词</span><h4>批量应用提示词</h4></div><span>{selectedPromptVersion ? `v${selectedPromptVersion.version}` : "未选择"}</span></div>
            <label><span>提示词条目</span><select value={promptEntryId} onChange={(event) => setPromptEntryId(event.target.value)} disabled={isBusy || !promptEntries.length}><option value="">选择模板</option>{promptEntries.map((entry) => <option key={entry.id} value={entry.id}>{entry.name}</option>)}</select></label>
            <label><span>版本</span><select value={selectedPromptVersion?.id ?? ""} onChange={(event) => setPromptVersionId(event.target.value)} disabled={isBusy || !selectedPromptEntry}>{selectedPromptEntry?.versions.map((version) => <option key={version.id} value={version.id}>v{version.version} · {version.text.slice(0, 48)}</option>)}</select></label>
            {referenceAnchors.length > 0 && <div className="episode-production-anchor-list"><span>上下文锚点（不改变素材关系）</span>{referenceAnchors.slice(0, 20).map((anchor) => <label key={anchor.id}><input type="checkbox" checked={anchorIds.includes(anchor.id)} onChange={() => setAnchorIds((current) => current.includes(anchor.id) ? current.filter((id) => id !== anchor.id) : [...current, anchor.id])} disabled={isBusy} />{anchor.name}</label>)}</div>}
            {promptCustomNames.length > 0 && <div className="episode-production-custom-values">{promptCustomNames.map((name) => <label key={name}><span>{name}</span><input value={customValues[name] ?? ""} onChange={(event) => setCustomValues((current) => ({ ...current, [name]: event.target.value }))} disabled={isBusy} /></label>)}</div>}
            <div className="episode-production-actions"><button type="button" onClick={() => void previewPrompt()} disabled={isBusy || !selectedPromptVersion || !selectedShotIds.length}>{busyAction === "prompt-preview" ? "预览中…" : "预览所选场景提示词"}</button><button type="button" className="quiet-button" onClick={() => void applyPrompt()} disabled={isBusy || !selectedPromptVersion || !promptPreview || promptPreview.invalid > 0}>应用所选场景提示词</button></div>
            {promptPreview && <p className={promptPreview.invalid ? "episode-production-inline-error" : "episode-production-inline-success"}>预览：{promptPreview.valid}/{promptPreview.total} 可用{promptPreview.invalid ? `，${promptPreview.invalid} 个阻塞` : ""}。每个镜头使用自己的场景上下文。</p>}
          </section>
        </div>

        <section className="episode-production-card episode-production-prepare" aria-label="集准备">
          <div className="episode-production-card-heading"><div><span className="section-label">准备</span><h4>准备所选场景</h4></div><span>{selectedShotIds.length} 个镜头</span></div>
          <label className="episode-production-partial-toggle"><input type="checkbox" checked={allowPartial} onChange={(event) => setAllowPartial(event.target.checked)} disabled={isBusy} /> 跳过阻塞场景，仅准备当前可生产内容</label>
          <p className="episode-production-hint">默认严格模式；确认文字会明确提示“不会自动启动 GPU”。每个场景最多准备一个就绪批次。</p>
          <button type="button" className="episode-production-primary-action" onClick={() => void prepare()} disabled={isBusy || !selectedPlans.length || !selectedShotIds.length}>{busyAction === "prepare" ? "准备中…" : "准备所选场景"}</button>
        </section>
      </> : <div className="episode-production-loading" role="status">正在加载集生产计划…</div>}

      {result && <EpisodePrepareResultView result={result} onOpenProductionQueue={onOpenProductionQueue} disabled={isBusy} onNavigateToScene={onNavigateToScene} />}
      {error && <div className="episode-production-error" role="alert"><strong>{error.code}</strong><span>{error.message}</span>{error.technicalMessage && <details><summary>技术详情</summary><code>{error.technicalMessage}</code></details>}</div>}
      {notice && <div className="episode-production-notice" role="status">{notice}</div>}
    </section>
  );
}

export function EpisodePrepareResultView({
  result,
  onOpenProductionQueue,
  disabled,
  onNavigateToScene,
}: {
  result: EpisodeProductionPrepareResult;
  onOpenProductionQueue?: () => void;
  disabled: boolean;
  onNavigateToScene?: (sceneId: string) => void;
}) {
  const status = result.status ?? (result.skippedBlockedScenes.length > 0 ? "PARTIAL" : result.createdBatches > 0 ? "SUCCESS" : "NOOP");
  const blockerRows = result.results.filter((row) => row.blockingReasons.length > 0 || row.status === "BLOCKED" || Boolean(row.error));
  return <section className={`episode-production-result episode-production-result-${status.toLowerCase()}`} aria-label="集准备结果">
    <div><span className="section-label">准备结果</span><h4>{episodePrepareStatusLabel(status)}</h4></div>
    <p>创建批次：<strong>{result.createdBatches}</strong> · 新建镜头项目：<strong>{result.createdItems}</strong> · 已跳过：<strong>{result.alreadyPreparedScenes.length + result.skippedDoneScenes.length + result.skippedEmptyScenes.length}</strong></p>
    {status === "PARTIAL" && <p>已成功准备部分场景，其余场景因状态变化或阻塞未准备。</p>}
    {blockerRows.length > 0 && <div className="episode-production-blockers"><strong>阻塞场景</strong>{blockerRows.map((row) => <div key={row.sceneId}><span>{row.sceneName || row.sceneId}：{row.blockingReasons.join("；") || row.error || "状态阻塞"}</span><button type="button" className="quiet-button" onClick={() => onNavigateToScene?.(row.sceneId)} disabled={disabled}>查看场景</button></div>)}</div>}
    <button type="button" onClick={onOpenProductionQueue} disabled={disabled || !onOpenProductionQueue}>打开生产队列</button>
  </section>;
}

export function productionEpisodeOptions(tree: EpisodeProductionPanelProps["tree"]): Array<{ value: string; label: string; seriesName: string }> {
  return orderedSeries(tree).flatMap((series) => orderedEpisodes(series).map((episode) => ({
    value: episode.id,
    label: `${series.name} / 第${String(episode.ordinal + 1).padStart(2, "0")}集 · ${episode.name}`,
    seriesName: series.name,
  })));
}

export function episodeSceneShotIds(tree: EpisodeProductionPanelProps["tree"], shots: EpisodeProductionPanelProps["shots"], sceneIds: string[]): string[] {
  const selected = new Set(sceneIds);
  const shotIds = new Set(orderedSeries(tree).flatMap((series) => orderedEpisodes(series).flatMap((episode) => orderedScenes(episode).filter((scene) => selected.has(scene.id)).flatMap((scene) => scene.shotIds))));
  return [...shotIds].sort((left, right) => (shots.find((shot) => shot.id === left)?.ordinal ?? Number.MAX_SAFE_INTEGER) - (shots.find((shot) => shot.id === right)?.ordinal ?? Number.MAX_SAFE_INTEGER) || left.localeCompare(right));
}

export function matchesEpisodeFilter(scene: EpisodeProductionScenePlan, filter: EpisodeProductionFilter): boolean {
  if (filter === "all") return true;
  if (filter === "ready") return scene.canPrepare;
  if (filter === "prepared") return scene.classification === "PREPARED";
  if (filter === "blocked") return scene.blocked > 0;
  return scene.classification === "DONE";
}

export function episodeSceneClassificationLabel(classification: EpisodeProductionSceneClassification): string {
  return { DONE: "已完成", PREPARED: "已准备", READY: "可准备", PARTIAL: "部分可准备", BLOCKED: "被阻塞", EMPTY: "空场景" }[classification];
}

export function episodeFilterLabel(filter: EpisodeProductionFilter): string {
  return { all: "全部", ready: "可准备", prepared: "已准备", blocked: "有阻塞", done: "已完成" }[filter];
}

export function episodeProductionActionDisabled(busyAction?: BusyAction | string): boolean {
  return Boolean(busyAction);
}

export function episodePresetConfirmation(stage: EpisodeProductionStage, sceneCount: number, shotCount: number): string {
  return `将覆盖 ${sceneCount} 个场景、${shotCount} 个镜头的${sceneProductionStageLabel(stage)}阶段配置。引用素材和已选媒体不会改变。是否继续？`;
}

export function episodePrepareConfirmation(episodeName: string, stage: EpisodeProductionStage, sceneCount: number, shotCount: number): string {
  return `将为“${episodeName}”的 ${sceneCount} 个场景准备${sceneProductionStageLabel(stage)}生产批次，共 ${shotCount} 个镜头。不会自动启动 GPU。是否继续？`;
}

function blockedPrepareResult(projectId: string, episodeId: string, stage: EpisodeProductionStage, scenes: EpisodeProductionScenePlan[]): EpisodeProductionPrepareResult {
  const blockers = scenes.filter((scene) => scene.blocked > 0);
  return {
    projectId,
    episodeId,
    stage,
    status: "BLOCKED",
    requestedScenes: scenes.length,
    createdBatches: 0,
    createdItems: 0,
    alreadyPreparedScenes: [],
    skippedDoneScenes: [],
    skippedEmptyScenes: [],
    skippedBlockedScenes: blockers.map((scene) => scene.sceneId),
    results: blockers.map((scene) => ({ sceneId: scene.sceneId, sceneName: scene.sceneName, status: "BLOCKED", created: false, createdCount: 0, existingBatchIds: scene.existingBatchIds, blockingReasons: scene.blockingReasons })),
  };
}

function episodePrepareStatusLabel(status: "SUCCESS" | "NOOP" | "PARTIAL" | "BLOCKED"): string {
  return { SUCCESS: "已准备", NOOP: "没有新增批次", PARTIAL: "部分准备", BLOCKED: "未创建批次" }[status];
}

function episodePrepareNotice(result: EpisodeProductionPrepareResult): string {
  const status = result.status ?? (result.createdBatches > 0 ? "SUCCESS" : "NOOP");
  if (status === "PARTIAL") return `已成功准备部分场景，创建 ${result.createdBatches} 个生产批次；其余场景因状态变化未准备。`;
  if (status === "BLOCKED") return "严格模式未创建批次；请处理阻塞场景后重新检查。";
  return `已准备 ${result.createdBatches} 个生产批次、${result.createdItems} 个镜头项目；不会自动启动 GPU。`;
}

function toEpisodeError(value: unknown, fallbackCode: string): EpisodeError {
  const formatted = formatUiError(value);
  const embeddedCode = formatted.technicalMessage?.match(/EPISODE_(?:PRODUCTION|BULK|SCENE)[A-Z0-9_]*/)?.[0];
  return { code: embeddedCode ?? formatted.code ?? fallbackCode, message: embeddedCode === "EPISODE_PRODUCTION_BLOCKED" ? "严格模式未创建批次；请先处理阻塞场景。" : formatted.message, technicalMessage: formatted.technicalMessage };
}

function parseEpisodePrepareErrorResult(value: unknown): EpisodeProductionPrepareResult | undefined {
  const technicalMessage = formatUiError(value).technicalMessage ?? "";
  const marker = "EPISODE_PRODUCTION_PARTIAL:";
  const markerIndex = technicalMessage.indexOf(marker);
  if (markerIndex < 0) return undefined;
  try {
    return JSON.parse(technicalMessage.slice(markerIndex + marker.length).trim()) as EpisodeProductionPrepareResult;
  } catch {
    return undefined;
  }
}
