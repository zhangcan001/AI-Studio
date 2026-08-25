import { useEffect, useMemo, useState } from "react";
import {
  applyPromptTemplate,
  bulkSetShotStageConfig,
  createBatchWorkflowPreset,
  deleteBatchWorkflowPreset,
  getSceneProductionPlan,
  listBatchWorkflowPresets,
  prepareSceneProduction,
  previewPromptTemplateBulk,
  startProductionQueue,
  updateBatchWorkflowPreset,
} from "../../services/tauriClient";
import { formatUiError } from "../../i18n/errorMessages";
import type {
  BatchWorkflowPreset,
  BatchWorkflowPresetCreateRequest,
  SceneProductionClassification,
  SceneProductionPanelProps,
  SceneProductionPlan,
  SceneProductionStage,
} from "../../types/sceneProduction";
import {
  sanitizeReusableGenerationValues,
  sceneProductionClassificationLabel,
  sceneProductionStageLabel,
  sceneProductionStagePreset,
} from "../../types/sceneProduction";
import type { ShotInputValues } from "../../types/shot";
import { analyzePromptTemplateText, customPromptVariableNames } from "../prompts/promptTemplateState";
import "./SceneProductionPanel.css";

type BusyAction = "load" | "preset-save" | "preset-rename" | "preset-delete" | "preset-apply" | "prompt-preview" | "prompt-apply" | "plan" | "prepare" | "start";

interface PanelError {
  code: string;
  message: string;
  technicalMessage?: string;
}

const STAGES: SceneProductionStage[] = ["image", "video"];
const CLASSIFICATIONS: SceneProductionClassification[] = ["DONE", "PREPARED", "ELIGIBLE", "BLOCKED"];

export function SceneProductionPanel({
  projectId,
  sceneOptions,
  currentSceneId,
  currentShot,
  promptEntries = [],
  referenceAnchors = [],
  initialPresets = [],
  initialPlan,
  onRefresh,
  onNotice,
  onNavigateToReview,
}: SceneProductionPanelProps) {
  const [sceneId, setSceneId] = useState(currentSceneId ?? initialPlan?.sceneId ?? sceneOptions[0]?.value ?? "");
  const [stage, setStage] = useState<SceneProductionStage>(initialPlan?.stage ?? "image");
  const [presets, setPresets] = useState<BatchWorkflowPreset[]>(initialPresets);
  const [selectedPresetId, setSelectedPresetId] = useState(initialPresets[0]?.id ?? "");
  const [plan, setPlan] = useState<SceneProductionPlan>(initialPlan ?? emptyPlan(projectId, sceneId, stage));
  const [busyAction, setBusyAction] = useState<BusyAction>();
  const [error, setError] = useState<PanelError>();
  const [notice, setNotice] = useState<string>();
  const [presetName, setPresetName] = useState("");
  const [presetDescription, setPresetDescription] = useState("");
  const [presetStages, setPresetStages] = useState<Set<SceneProductionStage>>(new Set(STAGES));
  const [promptEntryId, setPromptEntryId] = useState("");
  const [promptVersionId, setPromptVersionId] = useState("");
  const [anchorIds, setAnchorIds] = useState<string[]>([]);
  const [customValues, setCustomValues] = useState<Record<string, string>>({});
  const [promptPreview, setPromptPreview] = useState<{ total: number; valid: number; invalid: number }>();
  const [allowPartial, setAllowPartial] = useState(false);
  const [preparedBatchId, setPreparedBatchId] = useState<string>();

  const selectedPreset = presets.find((preset) => preset.id === selectedPresetId);
  const selectedStagePreset = sceneProductionStagePreset(selectedPreset, stage);
  const selectedPromptEntry = promptEntries.find((entry) => entry.id === promptEntryId);
  const selectedPromptVersion = selectedPromptEntry?.versions.find((version) => version.id === promptVersionId)
    ?? selectedPromptEntry?.versions[selectedPromptEntry.versions.length - 1];
  const promptCustomNames = useMemo(
    () => selectedPromptVersion ? customPromptVariableNames(analyzePromptTemplateText(selectedPromptVersion.text).customVariables) : [],
    [selectedPromptVersion],
  );
  const planShotIds = useMemo(() => plan.rows.map((row) => row.shotId), [plan.rows]);
  const isBusy = sceneProductionActionDisabled(busyAction);
  const canPrepare = plan.canPrepare && plan.eligible > 0;
  const partialCanPrepare = plan.eligible > 0 && plan.eligible <= plan.maxBatchItems;

  useEffect(() => {
    if (currentSceneId && sceneOptions.some((option) => option.value === currentSceneId)) {
      setSceneId(currentSceneId);
      return;
    }
    setSceneId((current) => sceneOptions.some((option) => option.value === current) ? current : sceneOptions[0]?.value ?? "");
  }, [currentSceneId, sceneOptions]);

  useEffect(() => {
    if (initialPresets.length) return;
    let active = true;
    void listBatchWorkflowPresets()
      .then((next) => {
        if (!active) return;
        setPresets(next);
        setSelectedPresetId((current) => current && next.some((preset) => preset.id === current) ? current : next[0]?.id ?? "");
      })
      .catch((value: unknown) => { if (active) setError(toPanelError(value, "BATCH_WORKFLOW_PRESETS_LOAD_FAILED")); });
    return () => { active = false; };
  }, [initialPresets.length, projectId]);

  useEffect(() => {
    if (!sceneId) return;
    let active = true;
    setBusyAction("plan");
    void getSceneProductionPlan({ projectId, sceneId, stage })
      .then((next) => { if (active) setPlan(next); })
      .catch((value: unknown) => { if (active) setError(toPanelError(value, "SCENE_PRODUCTION_PLAN_FAILED")); })
      .finally(() => { if (active) setBusyAction(undefined); });
    return () => { active = false; };
  }, [projectId, sceneId, stage]);

  useEffect(() => {
    const entry = promptEntries[0];
    setPromptEntryId((current) => current && promptEntries.some((item) => item.id === current) ? current : entry?.id ?? "");
  }, [promptEntries]);

  useEffect(() => {
    if (!selectedPreset) return;
    setPresetName(selectedPreset.name);
    setPresetDescription(selectedPreset.description);
  }, [selectedPresetId]);

  useEffect(() => {
    const latest = selectedPromptEntry?.versions[selectedPromptEntry.versions.length - 1];
    setPromptVersionId((current) => current && selectedPromptEntry?.versions.some((version) => version.id === current) ? current : latest?.id ?? "");
  }, [selectedPromptEntry]);

  useEffect(() => {
    setCustomValues((current) => Object.fromEntries(promptCustomNames.map((name) => [name, current[name] ?? ""])));
    setPromptPreview(undefined);
  }, [promptCustomNames]);

  function clearFeedback() {
    setError(undefined);
    setNotice(undefined);
  }

  async function runAction(action: BusyAction, callback: () => Promise<void>) {
    if (busyAction) return;
    setBusyAction(action);
    clearFeedback();
    try {
      await callback();
    } catch (value: unknown) {
      setError(toPanelError(value, "SCENE_PRODUCTION_ERROR"));
    } finally {
      setBusyAction(undefined);
    }
  }

  async function refreshPlan() {
    if (!sceneId) return;
    await runAction("plan", async () => {
      await reloadPlan();
    });
  }

  async function reloadPlan() {
    if (!sceneId) return;
    setPlan(await getSceneProductionPlan({ projectId, sceneId, stage }));
  }

  async function saveCurrentPreset() {
    if (!presetName.trim()) {
      setError({ code: "PRESET_NAME_REQUIRED", message: "请输入预设名称。" });
      return;
    }
    if (!currentShot) {
      setError({ code: "CURRENT_SHOT_REQUIRED", message: "请先在镜头工作区选择一个已配置镜头。" });
      return;
    }
    const configs = new Map(currentShot.stageConfigs.map((config) => [config.stage, config]));
    const missingStage = STAGES.find((nextStage) => presetStages.has(nextStage) && !configs.has(nextStage));
    if (missingStage) {
      setError({ code: "STAGE_CONFIG_REQUIRED", message: `当前镜头没有${sceneProductionStageLabel(missingStage)}阶段配置。` });
      return;
    }
    const request: BatchWorkflowPresetCreateRequest = {
      name: presetName.trim(),
      description: presetDescription.trim(),
      ...Object.fromEntries(STAGES.filter((nextStage) => presetStages.has(nextStage)).map((nextStage) => {
        const config = configs.get(nextStage);
        return [nextStage, {
          workflowVersionId: config!.workflowVersionId,
          recipeId: config!.recipeId,
          values: sanitizeReusableGenerationValues(config!.scalarValues as ShotInputValues),
        }];
      })),
    } as BatchWorkflowPresetCreateRequest;
    await runAction("preset-save", async () => {
      const created = await createBatchWorkflowPreset(request);
      setPresets((current) => [created, ...current.filter((preset) => preset.id !== created.id)]);
      setSelectedPresetId(created.id);
      setNotice(`预设“${created.name}”已保存；引用素材和已选媒体未写入预设。`);
      onNotice?.(`预设“${created.name}”已保存。`);
    });
  }

  async function renamePreset() {
    if (!selectedPreset || !presetName.trim()) return;
    await runAction("preset-rename", async () => {
      const updated = await updateBatchWorkflowPreset({
        presetId: selectedPreset.id,
        name: presetName.trim(),
        description: presetDescription.trim(),
        image: selectedPreset.image,
        video: selectedPreset.video,
      });
      setPresets((current) => current.map((preset) => preset.id === updated.id ? updated : preset));
      setNotice(`预设已重命名为“${updated.name}”。`);
    });
  }

  async function removePreset() {
    if (!selectedPreset || !window.confirm(`确定删除预设“${selectedPreset.name}”吗？`)) return;
    await runAction("preset-delete", async () => {
      await deleteBatchWorkflowPreset(selectedPreset.id);
      setPresets((current) => current.filter((preset) => preset.id !== selectedPreset.id));
      setSelectedPresetId("");
      setNotice("预设已删除。不会影响任何已有镜头配置。");
    });
  }

  async function applyPreset() {
    if (!selectedStagePreset || !selectedPreset || !planShotIds.length) {
      setError({ code: "PRESET_STAGE_UNAVAILABLE", message: `当前预设没有可用的${sceneProductionStageLabel(stage)}阶段配置。` });
      return;
    }
    if (!window.confirm(presetOverwriteConfirmation(stage, planShotIds.length))) return;
    await runAction("preset-apply", async () => {
      await bulkSetShotStageConfig({
        projectId,
        stage,
        shotIds: planShotIds,
        workflowVersionId: selectedStagePreset.workflowVersionId,
        recipeId: selectedStagePreset.recipeId,
        values: selectedStagePreset.values,
      });
      setNotice(`已将“${selectedPreset.name}”应用到场景的 ${planShotIds.length} 个镜头。`);
      await reloadPlan();
      await onRefresh?.();
    });
  }

  async function previewPrompt() {
    if (!selectedPromptEntry || !selectedPromptVersion || !planShotIds.length) return;
    await runAction("prompt-preview", async () => {
      const preview = await previewPromptTemplateBulk({
        projectId,
        promptEntryId: selectedPromptEntry.id,
        promptVersionId: selectedPromptVersion.id,
        shotIds: planShotIds,
        contextAnchorIds: anchorIds,
        customValues,
        previewLimit: 12,
      });
      setPromptPreview({ total: preview.total, valid: preview.valid, invalid: preview.invalid });
    });
  }

  async function applyPrompt() {
    if (!selectedPromptEntry || !selectedPromptVersion || !planShotIds.length || !promptPreview || promptPreview.invalid > 0) return;
    await runAction("prompt-apply", async () => {
      await applyPromptTemplate({
        projectId,
        promptEntryId: selectedPromptEntry.id,
        promptVersionId: selectedPromptVersion.id,
        stage,
        shotIds: planShotIds,
        contextAnchorIds: anchorIds,
        customValues,
      });
      setNotice(`已将提示词模板应用到 ${planShotIds.length} 个镜头；最终提示词已按阶段快照冻结。`);
      setPromptPreview(undefined);
      await reloadPlan();
      await onRefresh?.();
    });
  }

  async function prepare(allowPartialValue: boolean) {
    const canPrepareRequest = allowPartialValue ? partialCanPrepare : canPrepare;
    if (!sceneId || !planShotIds.length || !canPrepareRequest) return;
    await runAction("prepare", async () => {
      const result = await prepareSceneProduction({ projectId, sceneId, stage, allowPartial: allowPartialValue });
      setPreparedBatchId(result.batchId ?? result.existingBatchIds[0] ?? undefined);
      setNotice(result.created
        ? `已准备 ${sceneProductionStageLabel(stage)}生产批次；生成尚未启动，图片 → 视频仍需人工审核。`
        : result.alreadyPrepared ? "该场景阶段已有准备中的批次，未创建重复生产项。" : (result.message ?? "没有创建新的生产批次。"));
      await reloadPlan();
      await onRefresh?.();
    });
  }

  async function startPreparedBatch() {
    if (!preparedBatchId) {
      setError({ code: "PRODUCTION_BATCH_REQUIRED", message: "请先准备场景生产批次。" });
      return;
    }
    await runAction("start", async () => {
      await startProductionQueue(projectId, preparedBatchId);
      setNotice(`${sceneProductionStageLabel(stage)}生产已提交到现有生产队列；结果仍需人工审核。`);
      onNavigateToReview?.(stage);
    });
  }

  function selectPreset(id: string) {
    setSelectedPresetId(id);
    const next = presets.find((preset) => preset.id === id);
    setPresetName(next?.name ?? "");
    setPresetDescription(next?.description ?? "");
  }

  function selectStage(nextStage: SceneProductionStage) {
    setStage(nextStage);
    setPreparedBatchId(undefined);
    setPromptPreview(undefined);
  }

  if (!sceneOptions.length) {
    return <section className="scene-production-panel" aria-label="场景生产"><div className="scene-production-empty"><strong>暂无可生产场景</strong><span>请先在现有内容结构中创建场景并分配镜头。</span></div></section>;
  }

  return (
    <section className="scene-production-panel" aria-label="场景生产">
      <div className="scene-production-header">
        <div><span className="section-label">场景生产</span><h3>场景生产自动化</h3><p>配置、检查、准备，再由你明确启动现有生产队列。</p></div>
        <span className="scene-production-safety">不会自动生成或跨过人工审核</span>
      </div>

      <div className="scene-production-toolbar">
        <label><span>场景</span><select value={sceneId} onChange={(event) => setSceneId(event.target.value)} disabled={isBusy} aria-label="场景选择">{sceneOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>
        <div className="scene-production-stage-tabs" role="tablist" aria-label="生产阶段">{STAGES.map((nextStage) => <button key={nextStage} type="button" className={stage === nextStage ? "active" : ""} role="tab" aria-selected={stage === nextStage} onClick={() => selectStage(nextStage)} disabled={isBusy}>{sceneProductionStageLabel(nextStage)}</button>)}</div>
        <button type="button" className="quiet-button" onClick={() => void refreshPlan()} disabled={isBusy || !sceneId}>{busyAction === "plan" ? "检查中…" : "重新检查计划"}</button>
      </div>

      <div className="scene-production-grid">
        <section className="scene-production-card" aria-label="批量工作流预设">
          <div className="scene-production-card-heading"><div><span className="section-label">预设</span><h4>批量工作流预设</h4></div><span>{presets.length} / 30</span></div>
          <label><span>当前预设</span><select value={selectedPresetId} onChange={(event) => selectPreset(event.target.value)} disabled={isBusy}><option value="">选择预设</option>{presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}{!preset.available ? " · 不可用" : ""}</option>)}</select></label>
          {selectedPreset && <div className="scene-production-preset-summary"><strong>{selectedPreset.name}</strong><span className={selectedPreset.available ? "scene-production-available" : "scene-production-unavailable"}>{selectedPreset.available ? "可用" : selectedPreset.unavailableReason ?? selectedPreset.reason ?? "WORKFLOW_UNAVAILABLE"}</span><small>图片：{presetSummary(selectedPreset, "image")} · 视频：{presetSummary(selectedPreset, "video")}</small></div>}
          <div className="scene-production-form-grid"><label><span>名称</span><input value={presetName} maxLength={80} onChange={(event) => setPresetName(event.target.value)} placeholder="例如：电影感场景基础配置" disabled={isBusy} /></label><label><span>说明</span><input value={presetDescription} maxLength={500} onChange={(event) => setPresetDescription(event.target.value)} placeholder="可选，最多 500 字" disabled={isBusy} /></label></div>
          <div className="scene-production-checks"><span>从当前镜头保存：</span>{STAGES.map((nextStage) => <label key={nextStage}><input type="checkbox" checked={presetStages.has(nextStage)} onChange={() => setPresetStages((current) => toggleStage(current, nextStage))} disabled={isBusy} />{sceneProductionStageLabel(nextStage)}</label>)}</div>
          <div className="scene-production-actions"><button type="button" onClick={() => void saveCurrentPreset()} disabled={isBusy || !currentShot || !presetStages.size}>{busyAction === "preset-save" ? "保存中…" : "保存当前配置为预设"}</button><button type="button" className="quiet-button" onClick={() => void renamePreset()} disabled={isBusy || !selectedPreset || !presetName.trim()}>重命名</button><button type="button" className="quiet-button danger-button" onClick={() => void removePreset()} disabled={isBusy || !selectedPreset}>删除</button></div>
          <button type="button" className="scene-production-secondary-action" onClick={() => void applyPreset()} disabled={isBusy || !selectedStagePreset || !selectedPreset?.available || !planShotIds.length}>{busyAction === "preset-apply" ? "应用中…" : `应用${sceneProductionStageLabel(stage)}预设到场景`}</button>
          <small className="scene-production-hint">应用会覆盖阶段配置，但不会覆盖有序参考图、已确认图片或已确认视频。</small>
        </section>

        <section className="scene-production-card" aria-label="提示词模板批量应用">
          <div className="scene-production-card-heading"><div><span className="section-label">提示词</span><h4>提示词模板</h4></div><span>{selectedPromptVersion ? `v${selectedPromptVersion.version}` : "未选择"}</span></div>
          <label><span>提示词条目</span><select value={promptEntryId} onChange={(event) => setPromptEntryId(event.target.value)} disabled={isBusy || !promptEntries.length}><option value="">选择模板</option>{promptEntries.map((entry) => <option key={entry.id} value={entry.id}>{entry.name}</option>)}</select></label>
          <label><span>版本</span><select value={selectedPromptVersion?.id ?? ""} onChange={(event) => setPromptVersionId(event.target.value)} disabled={isBusy || !selectedPromptEntry}>{selectedPromptEntry?.versions.map((version) => <option key={version.id} value={version.id}>v{version.version} · {version.text.slice(0, 48)}</option>)}</select></label>
          {referenceAnchors.length > 0 && <div className="scene-production-anchor-list"><span>上下文锚点（不改变素材关系）</span>{referenceAnchors.slice(0, 20).map((anchor) => <label key={anchor.id}><input type="checkbox" checked={anchorIds.includes(anchor.id)} onChange={() => setAnchorIds((current) => current.includes(anchor.id) ? current.filter((id) => id !== anchor.id) : [...current, anchor.id])} disabled={isBusy} />{anchor.name}</label>)}</div>}
          {promptCustomNames.length > 0 && <div className="scene-production-custom-values">{promptCustomNames.map((name) => <label key={name}><span>{name}</span><input value={customValues[name] ?? ""} maxLength={4096} onChange={(event) => setCustomValues((current) => ({ ...current, [name]: event.target.value }))} disabled={isBusy} /></label>)}</div>}
          <div className="scene-production-actions"><button type="button" onClick={() => void previewPrompt()} disabled={isBusy || !selectedPromptVersion || !planShotIds.length}>{busyAction === "prompt-preview" ? "预览中…" : "预览场景提示词"}</button><button type="button" className="quiet-button" onClick={() => void applyPrompt()} disabled={isBusy || !selectedPromptVersion || !promptPreview || promptPreview.invalid > 0}>{busyAction === "prompt-apply" ? "应用中…" : `应用${sceneProductionStageLabel(stage)}提示词`}</button></div>
          {promptPreview && <p className={promptPreview.invalid ? "scene-production-inline-error" : "scene-production-inline-success"}>预览：{promptPreview.valid}/{promptPreview.total} 可用{promptPreview.invalid ? `，${promptPreview.invalid} 个阻塞` : ""}。应用后会冻结最终阶段提示词。</p>}
        </section>
      </div>

      <section className="scene-production-card scene-production-plan" aria-label="生产计划">
        <div className="scene-production-card-heading"><div><span className="section-label">生产计划</span><h4>{plan.sceneName || "当前场景"} · {sceneProductionStageLabel(stage)}</h4></div><span>{plan.total} 个镜头</span></div>
        <div className="scene-production-counts">{CLASSIFICATIONS.map((classification) => <span key={classification} className={`scene-production-count scene-production-count-${classification.toLowerCase()}`}><strong>{plan[classification.toLowerCase() as "done" | "prepared" | "eligible" | "blocked"]}</strong>{sceneProductionClassificationLabel(classification)}</span>)}</div>
        {!plan.total && <div className="scene-production-empty"><strong>场景暂无镜头</strong><span>请回到现有内容结构分配镜头。</span></div>}
        {plan.total > 0 && <div className="scene-production-table-wrap"><table><thead><tr><th>镜头</th><th>分类</th><th>阻塞原因</th></tr></thead><tbody>{plan.rows.slice(0, 100).map((row) => <tr key={row.shotId}><td><strong>{String(row.globalOrdinal + 1).padStart(2, "0")} · {row.name}</strong><small>{row.shotId}</small></td><td><span className={`scene-production-classification scene-production-classification-${row.classification.toLowerCase()}`}>{sceneProductionClassificationLabel(row.classification)}</span></td><td>{row.blockingReasons.length ? row.blockingReasons.join("；") : row.existingBatchId ? `已有批次 ${row.existingBatchId}` : "—"}</td></tr>)}</tbody></table></div>}
        <div className="scene-production-prepare-bar"><label><input type="checkbox" checked={allowPartial} onChange={(event) => setAllowPartial(event.target.checked)} disabled={isBusy} /> 仅准备当前可生产镜头（跳过被阻塞项）</label><span>单批最多 {plan.maxBatchItems} 个</span><button type="button" onClick={() => void prepare(false)} disabled={isBusy || !canPrepare}>{busyAction === "prepare" ? "准备中…" : "准备场景生产"}</button><button type="button" className="quiet-button" onClick={() => void prepare(true)} disabled={isBusy || !allowPartial || !partialCanPrepare}>{busyAction === "prepare" ? "准备中…" : "仅准备可生产镜头"}</button><button type="button" className="scene-production-start" onClick={() => void startPreparedBatch()} disabled={isBusy || !preparedBatchId}>{busyAction === "start" ? "启动中…" : "启动生产"}</button></div>
        <p className="scene-production-hint">准备不会启动 GPU。图片完成后必须人工选择已确认图片，之后视频计划才会自然变为可生产；视频结果同样需要人工审核。</p>
      </section>

      {error && <div className="scene-production-error" role="alert"><strong>{error.code}</strong><span>{error.message}</span>{error.technicalMessage && <details><summary>技术详情</summary><code>{error.technicalMessage}</code></details>}</div>}
      {notice && <div className="scene-production-notice" role="status">{notice}</div>}
    </section>
  );
}

function emptyPlan(projectId: string, sceneId: string, stage: SceneProductionStage): SceneProductionPlan {
  return { projectId, sceneId, sceneName: "", stage, total: 0, done: 0, prepared: 0, eligible: 0, blocked: 0, canPrepare: false, maxBatchItems: 100, rows: [] };
}

function toggleStage(current: Set<SceneProductionStage>, stage: SceneProductionStage): Set<SceneProductionStage> {
  const next = new Set(current);
  if (next.has(stage)) next.delete(stage);
  else next.add(stage);
  return next;
}

function presetSummary(preset: BatchWorkflowPreset, stage: SceneProductionStage): string {
  const config = sceneProductionStagePreset(preset, stage);
  if (!config) return "未保存";
  return `${config.workflowVersionId} / ${config.recipeId}`;
}

export function presetOverwriteConfirmation(stage: SceneProductionStage, shotCount: number): string {
  return `将覆盖场景内 ${shotCount} 个镜头的${sceneProductionStageLabel(stage)}阶段配置。引用素材和已选媒体不会改变。是否继续？`;
}

export function sceneProductionActionDisabled(busyAction?: string): boolean {
  return Boolean(busyAction);
}

function toPanelError(value: unknown, fallbackCode: string): PanelError {
  const formatted = formatUiError(value);
  return { code: formatted.code ?? fallbackCode, message: formatted.message, technicalMessage: formatted.technicalMessage };
}
