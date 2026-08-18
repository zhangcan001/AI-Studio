import { useEffect, useMemo, useState } from "react";
import type { BatchWorkflowPreset } from "../../types/sceneProduction";
import type {
  SeriesProductionBusyAction,
  SeriesProductionEpisodeClassification,
  SeriesProductionFilter,
  SeriesProductionPanelProps,
  SeriesProductionPlan,
  SeriesProductionPrepareResult,
  SeriesProductionPrepareStatus,
  SeriesProductionStage,
  SeriesPromptPreview,
} from "../../types/seriesProduction";
import { orderedEpisodes, orderedScenes, orderedSeries } from "./productionStructureState";
import { sceneProductionStageLabel } from "../../types/sceneProduction";
import "./SeriesProductionPanel.css";

const STAGES: SeriesProductionStage[] = ["image", "video"];
const FILTERS: SeriesProductionFilter[] = ["all", "ready", "blocked", "prepared", "done"];

type SeriesError = { code: string; message: string };

export function SeriesProductionPanel({
  projectId,
  tree,
  shots,
  promptEntries = [],
  referenceAnchors = [],
  initialPresets = [],
  initialPlan,
  onPlan,
  onPrepare,
  onApplyPreset,
  onPreviewPrompt,
  onApplyPrompt,
  onRefresh,
  onNotice,
  onError,
  onOpenProductionQueue,
  onOpenRunbook,
  onNavigateToEpisode,
}: SeriesProductionPanelProps) {
  const seriesOptions = useMemo(() => productionSeriesOptions(tree), [tree]);
  const [seriesId, setSeriesId] = useState(initialPlan?.seriesId ?? seriesOptions[0]?.value ?? "");
  const [stage, setStage] = useState<SeriesProductionStage>(initialPlan?.stage ?? "image");
  const [plan, setPlan] = useState<SeriesProductionPlan | undefined>(initialPlan);
  const [selectedEpisodeIds, setSelectedEpisodeIds] = useState<string[]>([]);
  const [filter, setFilter] = useState<SeriesProductionFilter>("all");
  const [allowPartial, setAllowPartial] = useState(false);
  const [busyAction, setBusyAction] = useState<SeriesProductionBusyAction>();
  const [error, setError] = useState<SeriesError>();
  const [notice, setNotice] = useState<string>();
  const [result, setResult] = useState<SeriesProductionPrepareResult>();
  const [presets, setPresets] = useState<BatchWorkflowPreset[]>(initialPresets);
  const [selectedPresetId, setSelectedPresetId] = useState(initialPresets[0]?.id ?? "");
  const [promptEntryId, setPromptEntryId] = useState(promptEntries[0]?.id ?? "");
  const [promptVersionId, setPromptVersionId] = useState("");
  const [anchorIds, setAnchorIds] = useState<string[]>([]);
  const [customValues, setCustomValues] = useState<Record<string, string>>({});
  const [promptPreview, setPromptPreview] = useState<SeriesPromptPreview>();

  const isBusy = Boolean(busyAction);
  const selectedEpisodePlans = useMemo(
    () => (plan?.episodes ?? []).filter((episode) => selectedEpisodeIds.includes(episode.episodeId)),
    [plan, selectedEpisodeIds],
  );
  const selectedSceneIds = useMemo(
    () => seriesEpisodeSceneIds(tree, selectedEpisodeIds),
    [tree, selectedEpisodeIds],
  );
  const selectedShotIds = useMemo(
    () => seriesEpisodeShotIds(tree, shots, selectedEpisodeIds),
    [tree, shots, selectedEpisodeIds],
  );
  const visibleEpisodes = useMemo(
    () => (plan?.episodes ?? []).filter((episode) => matchesSeriesFilter(episode, filter)),
    [filter, plan],
  );
  const selectedPreset = presets.find((preset) => preset.id === selectedPresetId);
  const selectedPromptEntry = promptEntries.find((entry) => entry.id === promptEntryId);
  const selectedPromptVersion = selectedPromptEntry?.versions.find((version) => version.id === promptVersionId)
    ?? selectedPromptEntry?.versions[selectedPromptEntry.versions.length - 1];

  useEffect(() => {
    if (seriesId && seriesOptions.some((option) => option.value === seriesId)) return;
    setSeriesId(seriesOptions[0]?.value ?? "");
  }, [seriesId, seriesOptions]);

  useEffect(() => {
    if (!onPlan || !seriesId) return;
    if (initialPlan?.seriesId === seriesId && initialPlan.stage === stage) return;
    let active = true;
    setBusyAction("plan");
    setError(undefined);
    void onPlan({ projectId, seriesId, stage })
      .then((next) => { if (active) setPlan(next); })
      .catch((value: unknown) => { if (active) setError(toSeriesError(value, "SERIES_PRODUCTION_PLAN_FAILED")); })
      .finally(() => { if (active) setBusyAction(undefined); });
    return () => { active = false; };
  }, [initialPlan, onPlan, projectId, seriesId, stage]);

  useEffect(() => {
    setSelectedEpisodeIds((current) => current.filter((episodeId) => plan?.episodes.some((episode) => episode.episodeId === episodeId) ?? false));
  }, [plan]);

  useEffect(() => {
    if (initialPresets.length) return;
    setPresets([]);
  }, [initialPresets.length]);

  useEffect(() => {
    setPromptEntryId((current) => current && promptEntries.some((entry) => entry.id === current) ? current : promptEntries[0]?.id ?? "");
  }, [promptEntries]);

  useEffect(() => {
    const latest = selectedPromptEntry?.versions[selectedPromptEntry.versions.length - 1];
    setPromptVersionId((current) => current && selectedPromptEntry?.versions.some((version) => version.id === current) ? current : latest?.id ?? "");
  }, [selectedPromptEntry]);

  function clearFeedback() {
    setError(undefined);
    setNotice(undefined);
    onError?.("");
  }

  async function runAction(action: SeriesProductionBusyAction, callback: () => Promise<void>) {
    if (busyAction) return;
    setBusyAction(action);
    clearFeedback();
    try {
      await callback();
    } catch (value: unknown) {
      const next = toSeriesError(value, "SERIES_PRODUCTION_ERROR");
      setError(next);
      onError?.(next.message);
    } finally {
      setBusyAction(undefined);
    }
  }

  async function refreshPlan() {
    if (!onPlan || !seriesId) return;
    await runAction("plan", async () => setPlan(await onPlan({ projectId, seriesId, stage })));
  }

  async function applyPreset(targetStage: SeriesProductionStage) {
    const stagePreset = selectedPreset?.[targetStage];
    if (!stagePreset || !selectedPreset || !selectedEpisodeIds.length || !onApplyPreset) return;
    if (selectedShotIds.length > 500) {
      setError({ code: "SERIES_BULK_SCOPE_TOO_LARGE", message: "所选集超过 500 个镜头，未执行批量应用。" });
      return;
    }
    if (!confirmIfAvailable(seriesPresetConfirmation(plan?.seriesName ?? "当前系列", selectedEpisodePlans.length, selectedSceneIds.length, selectedShotIds.length, targetStage))) return;
    await runAction("preset-apply", async () => {
      await onApplyPreset({
        projectId,
        seriesId,
        stage: targetStage,
        episodeIds: selectedEpisodeIds,
        sceneIds: selectedSceneIds,
        shotIds: selectedShotIds,
        presetId: selectedPreset.id,
        workflowVersionId: stagePreset.workflowVersionId,
        recipeId: stagePreset.recipeId,
        values: stagePreset.values,
      });
      setNotice(`已将“${selectedPreset.name}”应用到所选 ${selectedEpisodePlans.length} 集、${selectedSceneIds.length} 个场景、${selectedShotIds.length} 个镜头；引用素材和已选媒体未改变。`);
      onNotice?.("Series 预设已应用。");
      await refreshAfterMutation();
    });
  }

  function promptRequest() {
    if (!selectedPromptEntry || !selectedPromptVersion) return undefined;
    return {
      projectId,
      seriesId,
      stage,
      episodeIds: selectedEpisodeIds,
      sceneIds: selectedSceneIds,
      shotIds: selectedShotIds,
      promptEntryId: selectedPromptEntry.id,
      promptVersionId: selectedPromptVersion.id,
      contextAnchorIds: anchorIds,
      customValues,
    };
  }

  async function previewPrompt() {
    const request = promptRequest();
    if (!request || !onPreviewPrompt || !selectedShotIds.length) return;
    await runAction("prompt-preview", async () => setPromptPreview(await onPreviewPrompt(request)));
  }

  async function applyPrompt() {
    const request = promptRequest();
    if (!request || !onApplyPrompt || !promptPreview || promptPreview.invalid > 0 || !selectedShotIds.length) return;
    await runAction("prompt-apply", async () => {
      await onApplyPrompt(request);
      setPromptPreview(undefined);
      setNotice(`已将 Prompt 应用到 ${selectedEpisodePlans.length} 集、${selectedShotIds.length} 个镜头；每个镜头保留自己的 Series / Episode / Scene Context。`);
      await refreshAfterMutation();
    });
  }

  async function prepare() {
    if (!plan || !selectedEpisodeIds.length || !onPrepare) return;
    const strictBlockers = selectedEpisodePlans.filter((episode) => episode.classification === "BLOCKED" || episode.classification === "PARTIAL");
    if (!allowPartial && strictBlockers.length) {
      setResult(blockedPrepareResult(projectId, seriesId, stage, selectedEpisodePlans));
      setNotice("严格模式未创建批次：所选 Episode 存在阻塞或部分可准备内容。");
      return;
    }
    if (!confirmIfAvailable(seriesPrepareConfirmation(plan.seriesName, selectedEpisodePlans.length, selectedSceneIds.length, stage))) return;
    await runAction("prepare", async () => {
      const next = await onPrepare({ projectId, seriesId, stage, episodeIds: selectedEpisodeIds, allowPartial });
      setResult(next);
      setNotice(seriesPrepareNotice(next));
      await refreshAfterMutation();
    });
  }

  async function refreshAfterMutation() {
    await onRefresh?.();
    if (onPlan && seriesId) setPlan(await onPlan({ projectId, seriesId, stage }));
  }

  function selectAll(mode: "ready" | "blocked" | "clear") {
    if (mode === "clear") {
      setSelectedEpisodeIds([]);
      return;
    }
    setSelectedEpisodeIds((plan?.episodes ?? [])
      .filter((episode) => mode === "ready" ? episode.canPrepare : episode.blocked > 0)
      .map((episode) => episode.episodeId));
  }

  function toggleEpisode(episodeId: string) {
    setSelectedEpisodeIds((current) => current.includes(episodeId) ? current.filter((id) => id !== episodeId) : [...current, episodeId]);
  }

  function selectSeries(nextSeriesId: string) {
    setSeriesId(nextSeriesId);
    setPlan(undefined);
    setSelectedEpisodeIds([]);
    setResult(undefined);
  }

  function selectStage(nextStage: SeriesProductionStage) {
    setStage(nextStage);
    setPlan(undefined);
    setSelectedEpisodeIds([]);
    setResult(undefined);
    setPromptPreview(undefined);
  }

  if (!seriesOptions.length) {
    return <section className="series-production-panel" aria-label="系列生产规划"><div className="series-production-empty"><strong>暂无可生产系列</strong><span>请先在现有 Production Structure 中创建 Series、Episode 和 Scene。</span></div></section>;
  }

  const currentSeries = seriesOptions.find((option) => option.value === seriesId);
  return (
    <section className="series-production-panel" aria-label="系列生产规划" aria-busy={isBusy}>
      <div className="series-production-header">
        <div><span className="section-label">Series Production</span><h3>全季生产规划</h3><p>一次汇总多个 Episode；准备后仍由你按现有 Production Queue 手动启动。</p></div>
        <div className="series-production-header-actions"><span className="series-production-safety">不会自动启动 GPU · 不跨过人工 Review</span>{onOpenRunbook && <button type="button" className="quiet-button" onClick={onOpenRunbook} disabled={isBusy}>查看 Batch Runbook</button>}</div>
      </div>

      <div className="series-production-toolbar">
        <label><span>Series</span><select value={seriesId} onChange={(event) => selectSeries(event.target.value)} disabled={isBusy} aria-label="Series 选择">{seriesOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>
        <div className="series-production-stage-tabs" role="tablist" aria-label="生产阶段">{STAGES.map((nextStage) => <button key={nextStage} type="button" className={stage === nextStage ? "active" : ""} role="tab" aria-selected={stage === nextStage} onClick={() => selectStage(nextStage)} disabled={isBusy}>{sceneProductionStageLabel(nextStage)}</button>)}</div>
        <button type="button" className="quiet-button" onClick={() => void refreshPlan()} disabled={isBusy || !onPlan}>{busyAction === "plan" ? "检查中…" : "重新检查计划"}</button>
      </div>

      {plan ? <>
        <div className="series-production-summary" aria-label="Series 汇总">
          <div><span>系列</span><strong>{plan.seriesName || currentSeries?.label || "—"}</strong></div><div><span>Episodes</span><strong>{plan.episodeTotal}</strong></div><div><span>Scenes</span><strong>{plan.sceneTotal}</strong></div><div><span>Shots</span><strong>{plan.shotTotal}</strong></div><div><span>DONE</span><strong>{plan.done}</strong></div><div><span>PREPARED</span><strong>{plan.prepared}</strong></div><div><span>ELIGIBLE</span><strong>{plan.eligible}</strong></div><div><span>BLOCKED</span><strong>{plan.blocked}</strong></div><div><span>Ready Episodes</span><strong>{plan.readyEpisodeCount}</strong></div><div><span>Blocked Episodes</span><strong>{plan.blockedEpisodeCount}</strong></div><div><span>Completed Episodes</span><strong>{plan.completedEpisodeCount}</strong></div>
        </div>

        <section className="series-production-card" aria-label="Episode 选择">
          <div className="series-production-card-heading"><div><span className="section-label">Episode Scope</span><h4>Episode 总览</h4></div><span>{selectedEpisodeIds.length} 集 · {selectedSceneIds.length} 个场景 · {selectedShotIds.length} 个镜头</span></div>
          <div className="series-production-selection-actions"><div className="series-production-filter-tabs" role="group" aria-label="Episode 筛选">{FILTERS.map((value) => <button key={value} type="button" className={filter === value ? "active" : ""} onClick={() => setFilter(value)} disabled={isBusy}>{seriesFilterLabel(value)}</button>)}</div><div className="series-production-select-actions"><button type="button" className="quiet-button" onClick={() => selectAll("ready")} disabled={isBusy}>全选可准备</button><button type="button" className="quiet-button" onClick={() => selectAll("blocked")} disabled={isBusy}>全选有阻塞</button><button type="button" className="quiet-button" onClick={() => selectAll("clear")} disabled={isBusy}>清空</button></div></div>
          <div className="series-production-table-wrap"><table><thead><tr><th>选择</th><th>Episode</th><th>Scenes</th><th>Shots</th><th>DONE</th><th>PREPARED</th><th>ELIGIBLE</th><th>BLOCKED</th><th>状态</th><th /></tr></thead><tbody>{visibleEpisodes.map((episode) => <tr key={episode.episodeId}><td><input type="checkbox" checked={selectedEpisodeIds.includes(episode.episodeId)} onChange={() => toggleEpisode(episode.episodeId)} disabled={isBusy || episode.classification === "DONE" || episode.classification === "EMPTY"} aria-label={`选择集 ${episode.episodeName}`} /></td><td><strong>第 {String(episode.episodeOrdinal + 1).padStart(2, "0")} 集 · {episode.episodeName}</strong>{episode.blockingReasons.length > 0 && <small className="series-production-blocker-reason">{episode.blockingReasons.join("；")}</small>}</td><td>{episode.sceneTotal}</td><td>{episode.shotTotal}</td><td>{episode.done}</td><td>{episode.prepared}</td><td>{episode.eligible}</td><td>{episode.blocked}</td><td><span className={`series-production-status series-production-status-${episode.classification.toLowerCase()}`}>{seriesClassificationLabel(episode.classification)}</span></td><td>{episode.blocked > 0 && <button type="button" className="quiet-button" onClick={() => onNavigateToEpisode?.(episode.episodeId)} disabled={isBusy}>查看集</button>}</td></tr>)}</tbody></table></div>
          {!visibleEpisodes.length && <div className="series-production-empty"><strong>没有匹配的 Episode</strong><span>请切换筛选条件。</span></div>}
        </section>

        <div className="series-production-grid">
          <section className="series-production-card" aria-label="Series 批量预设"><div className="series-production-card-heading"><div><span className="section-label">Preset</span><h4>应用到所选集</h4></div><span>{presets.length} / 30</span></div><label><span>Batch Workflow Preset</span><select value={selectedPresetId} onChange={(event) => setSelectedPresetId(event.target.value)} disabled={isBusy}><option value="">选择预设</option>{presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}{!preset.available ? " · 不可用" : ""}</option>)}</select></label><div className="series-production-actions"><button type="button" onClick={() => { setStage("image"); void applyPreset("image"); }} disabled={isBusy || !selectedPreset?.image || !selectedEpisodeIds.length || !onApplyPreset}>{busyAction === "preset-apply" ? "应用中…" : "应用图片预设到所选集"}</button><button type="button" className="quiet-button" onClick={() => { setStage("video"); void applyPreset("video"); }} disabled={isBusy || !selectedPreset?.video || !selectedEpisodeIds.length || !onApplyPreset}>应用视频预设到所选集</button></div><small className="series-production-hint">按 Project global Shot ordinal 去重；不会改变 references、Selected Image、Selected Video、Anchors、Scene assignment 或 Shot ordinal。</small></section>

          <section className="series-production-card" aria-label="Series Prompt 批量应用"><div className="series-production-card-heading"><div><span className="section-label">Prompt</span><h4>应用到所选集</h4></div><span>{selectedPromptVersion ? `v${selectedPromptVersion.version}` : "未选择"}</span></div><label><span>Prompt Entry</span><select value={promptEntryId} onChange={(event) => setPromptEntryId(event.target.value)} disabled={isBusy || !promptEntries.length}><option value="">选择模板</option>{promptEntries.map((entry) => <option key={entry.id} value={entry.id}>{entry.name}</option>)}</select></label><label><span>Version</span><select value={selectedPromptVersion?.id ?? ""} onChange={(event) => setPromptVersionId(event.target.value)} disabled={isBusy || !selectedPromptEntry}><option value="">选择版本</option>{selectedPromptEntry?.versions.map((version) => <option key={version.id} value={version.id}>v{version.version} · {version.text.slice(0, 48)}</option>)}</select></label>{referenceAnchors.length > 0 && <div className="series-production-anchor-list"><span>Context Anchors（不改变素材关系）</span>{referenceAnchors.slice(0, 20).map((anchor) => <label key={anchor.id}><input type="checkbox" checked={anchorIds.includes(anchor.id)} onChange={() => setAnchorIds((current) => current.includes(anchor.id) ? current.filter((id) => id !== anchor.id) : [...current, anchor.id])} disabled={isBusy} />{anchor.name}</label>)}</div>}<label><span>Custom Values（按 Shot Context 解析）</span><input value={customValues.context ?? ""} onChange={(event) => setCustomValues((current) => ({ ...current, context: event.target.value }))} disabled={isBusy} placeholder="可选，具体变量由 Prompt 模板决定" /></label><div className="series-production-actions"><button type="button" onClick={() => void previewPrompt()} disabled={isBusy || !selectedPromptVersion || !selectedShotIds.length || !onPreviewPrompt}>{busyAction === "prompt-preview" ? "预览中…" : "预览所选集 Prompt"}</button><button type="button" className="quiet-button" onClick={() => void applyPrompt()} disabled={isBusy || !selectedPromptVersion || !selectedShotIds.length || !promptPreview || promptPreview.invalid > 0 || !onApplyPrompt}>应用所选集 Prompt</button></div>{promptPreview && <div className={promptPreview.invalid ? "series-production-inline-error" : "series-production-inline-success"}>预览：{promptPreview.valid}/{promptPreview.total} 可用{promptPreview.invalid ? `，${promptPreview.invalid} 个阻塞` : ""}。Series / Episode / Scene / Shot Context 按镜头分别解析。{promptPreview.samples?.slice(0, 3).map((sample) => <small key={sample.shotId}>{sample.shotId}：{sample.valid ? sample.text : sample.error ?? "无效"}</small>)}</div>}</section>
        </div>

        <section className="series-production-card series-production-prepare" aria-label="Series Prepare"><div className="series-production-card-heading"><div><span className="section-label">Prepare</span><h4>准备所选 Episode</h4></div><span>预计 {selectedSceneIds.length} 个场景 · 最多 {selectedSceneIds.length} 个 READY Batch</span></div><label className="series-production-partial-toggle"><input type="checkbox" checked={allowPartial} onChange={(event) => setAllowPartial(event.target.checked)} disabled={isBusy} /> 跳过阻塞内容，仅准备当前可生产场景</label><p className="series-production-hint">默认严格模式；严格模式遇到 BLOCKED/PARTIAL 时保持 0 mutation。不会自动启动 GPU，也不会自动启动下一批。</p><button type="button" className="series-production-primary-action" onClick={() => void prepare()} disabled={isBusy || !selectedEpisodeIds.length || !onPrepare}>{busyAction === "prepare" ? "准备中…" : "准备所选 Episode"}</button></section>
      </> : <div className="series-production-loading" role="status">正在加载 Series 生产计划…</div>}

      {result && <SeriesPrepareResultView result={result} onOpenProductionQueue={onOpenProductionQueue} disabled={isBusy} onNavigateToEpisode={onNavigateToEpisode} />}
      {error && <div className="series-production-error" role="alert"><strong>{error.code}</strong><span>{error.message}</span></div>}
      {notice && <div className="series-production-notice" role="status">{notice}</div>}
    </section>
  );
}

export function SeriesPrepareResultView({ result, onOpenProductionQueue, disabled, onNavigateToEpisode }: { result: SeriesProductionPrepareResult; onOpenProductionQueue?: () => void; disabled: boolean; onNavigateToEpisode?: (episodeId: string) => void }) {
  const status = result.status ?? (result.skippedBlockedEpisodes.length ? "PARTIAL" : result.createdBatches ? "SUCCESS" : "NOOP");
  const blockers = result.episodeResults.filter((row) => (row.blockingReasons?.length ?? 0) > 0 || Boolean(row.error) || row.status === "BLOCKED");
  const skipped = result.alreadyPreparedEpisodes.length + result.skippedDoneEpisodes.length + result.skippedEmptyEpisodes.length + result.skippedBlockedEpisodes.length;
  return <section className={`series-production-result series-production-result-${status.toLowerCase()}`} aria-label="Series Prepare 结果"><div><span className="section-label">Prepare Result</span><h4>{seriesPrepareStatusLabel(status)}</h4></div><p>成功创建：<strong>{result.createdBatches}</strong> 个 Batch · <strong>{result.createdItems}</strong> 个 Shot items · 已跳过：<strong>{skipped}</strong> 个 Episode</p>{status === "PARTIAL" && <p>部分 Episode 在准备期间状态发生变化，已创建内容不会回滚。</p>}{blockers.length > 0 && <div className="series-production-blockers"><strong>阻塞 Episode</strong>{blockers.map((row) => <div key={row.episodeId}><span>{row.episodeName || row.episodeId}：{row.blockingReasons?.join("；") || row.error || "状态阻塞"}</span><button type="button" className="quiet-button" onClick={() => onNavigateToEpisode?.(row.episodeId)} disabled={disabled}>查看集</button></div>)}</div>}<button type="button" onClick={onOpenProductionQueue} disabled={disabled || !onOpenProductionQueue}>打开生产队列</button></section>;
}

export function productionSeriesOptions(tree: SeriesProductionPanelProps["tree"]): Array<{ value: string; label: string }> {
  return orderedSeries(tree).map((series) => ({ value: series.id, label: `第${String(series.ordinal + 1).padStart(2, "0")}季 · ${series.name}` }));
}

export function seriesEpisodeSceneIds(tree: SeriesProductionPanelProps["tree"], episodeIds: string[]): string[] {
  const selected = new Set(episodeIds);
  return orderedSeries(tree).flatMap((series) => orderedEpisodes(series).filter((episode) => selected.has(episode.id)).flatMap((episode) => orderedScenes(episode).map((scene) => scene.id)));
}

export function seriesEpisodeShotIds(tree: SeriesProductionPanelProps["tree"], shots: SeriesProductionPanelProps["shots"], episodeIds: string[]): string[] {
  const selected = new Set(episodeIds);
  const shotIds = new Set(orderedSeries(tree).flatMap((series) => orderedEpisodes(series).filter((episode) => selected.has(episode.id)).flatMap((episode) => orderedScenes(episode).flatMap((scene) => scene.shotIds))));
  const ordinals = new Map(shots.map((shot) => [shot.id, shot.ordinal]));
  return [...shotIds].sort((left, right) => (ordinals.get(left) ?? Number.MAX_SAFE_INTEGER) - (ordinals.get(right) ?? Number.MAX_SAFE_INTEGER) || left.localeCompare(right));
}

export function matchesSeriesFilter(episode: SeriesProductionPlan["episodes"][number], filter: SeriesProductionFilter): boolean {
  if (filter === "all") return true;
  if (filter === "ready") return episode.canPrepare || episode.classification === "READY" || episode.classification === "PARTIAL";
  if (filter === "blocked") return episode.blocked > 0;
  if (filter === "prepared") return episode.classification === "PREPARED";
  return episode.classification === "DONE";
}

export function seriesClassificationLabel(classification: SeriesProductionEpisodeClassification): string {
  return { EMPTY: "EMPTY · 空", DONE: "DONE · 完成", PREPARED: "PREPARED · 已准备", READY: "READY · 可准备", PARTIAL: "PARTIAL · 部分可准备", BLOCKED: "BLOCKED · 阻塞" }[classification];
}

export function seriesFilterLabel(filter: SeriesProductionFilter): string {
  return { all: "全部", ready: "可准备", blocked: "有阻塞", prepared: "已准备", done: "已完成" }[filter];
}

export function seriesPrepareConfirmation(seriesName: string, episodeCount: number, sceneCount: number, stage: SeriesProductionStage): string {
  return `将为${seriesName}中的 ${episodeCount} 集、${sceneCount} 个场景准备${sceneProductionStageLabel(stage)}生产批次。预计最多创建 ${sceneCount} 个 READY 批次。不会自动启动 GPU。是否继续？`;
}

export function seriesPresetConfirmation(seriesName: string, episodeCount: number, sceneCount: number, shotCount: number, stage: SeriesProductionStage): string {
  return `将覆盖${seriesName}中 ${episodeCount} 集、${sceneCount} 个场景、${shotCount} 个镜头的${sceneProductionStageLabel(stage)}阶段配置。引用素材和已选媒体不会改变。是否继续？`;
}

export function seriesProductionActionDisabled(action: SeriesProductionBusyAction | undefined): boolean {
  return Boolean(action);
}

function blockedPrepareResult(projectId: string, seriesId: string, stage: SeriesProductionStage, episodes: SeriesProductionPlan["episodes"]): SeriesProductionPrepareResult {
  const blockers = episodes.filter((episode) => episode.classification === "BLOCKED" || episode.classification === "PARTIAL");
  return { projectId, seriesId, stage, status: "BLOCKED", requestedEpisodes: episodes.length, requestedScenes: blockers.reduce((total, episode) => total + episode.sceneTotal, 0), createdBatches: 0, createdItems: 0, alreadyPreparedEpisodes: [], skippedDoneEpisodes: [], skippedEmptyEpisodes: [], skippedBlockedEpisodes: blockers.map((episode) => episode.episodeId), episodeResults: blockers.map((episode) => ({ episodeId: episode.episodeId, episodeName: episode.episodeName, status: "BLOCKED", createdBatches: 0, createdItems: 0, alreadyPrepared: false, skipped: true, blockingReasons: episode.blockingReasons, batchIds: [] })) };
}

function seriesPrepareStatusLabel(status: SeriesProductionPrepareStatus): string {
  return { SUCCESS: "SUCCESS · 已准备", NOOP: "NOOP · 没有新增批次", PARTIAL: "PARTIAL · 部分准备", BLOCKED: "BLOCKED · 未创建批次" }[status];
}

function seriesPrepareNotice(result: SeriesProductionPrepareResult): string {
  const status = result.status ?? (result.createdBatches ? "SUCCESS" : "NOOP");
  if (status === "PARTIAL") return `部分 Episode 在准备期间状态发生变化，已创建 ${result.createdBatches} 个 Batch；已创建内容不会回滚。`;
  if (status === "BLOCKED") return "严格模式未创建批次；请处理阻塞 Episode 后重新检查。";
  return `已准备 ${result.createdBatches} 个 Batch、${result.createdItems} 个 Shot items；不会自动启动 GPU。`;
}

function toSeriesError(value: unknown, fallbackCode: string): SeriesError {
  if (typeof value === "object" && value !== null) {
    const candidate = value as { code?: unknown; message?: unknown };
    if (typeof candidate.code === "string" || typeof candidate.message === "string") return { code: typeof candidate.code === "string" ? candidate.code : fallbackCode, message: typeof candidate.message === "string" ? candidate.message : "Series 生产操作失败。" };
  }
  return { code: fallbackCode, message: value instanceof Error ? value.message : "Series 生产操作失败。" };
}

function confirmIfAvailable(message: string): boolean {
  return typeof window === "undefined" || typeof window.confirm !== "function" || window.confirm(message);
}
