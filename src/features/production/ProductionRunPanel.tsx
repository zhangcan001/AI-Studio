import { useEffect, useMemo, useState } from "react";
import {
  assetLibraryPage,
  cancelProductionRun,
  createProductionRun,
  getProductionRun,
  listProductionRuns,
  refreshProductionRun,
  retryProductionVideo,
  runProductionImages,
  runProductionVideo,
  selectProductionRunAssets,
} from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { GenerationValues, RecipeField, RecipeViewModel } from "../../types/generation";
import type { ProductionRun } from "../../types/productionRun";
import { toUserMessage } from "../../i18n/errorMessages";
import { defaultGenerationValues } from "../../stores/studioStore";

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  baseRecipe: RecipeViewModel;
  baseValues: GenerationValues;
  onOpenTask?: (taskId: string) => void;
  onAdmissionChanged?: () => Promise<void>;
}

function firstField(recipe: RecipeViewModel | undefined, predicate: (field: RecipeField) => boolean): RecipeField | undefined {
  return recipe?.fields.find(predicate);
}

function h3PromptKey(recipe: RecipeViewModel | undefined): string | undefined {
  return firstField(recipe, (field) => field.type === "textarea" && /prompt/i.test(field.key))?.key
    ?? firstField(recipe, (field) => field.type === "textarea")?.key;
}

function h3RecipeFor(catalog: RecipeViewModel[], baseRecipe: RecipeViewModel): RecipeViewModel | undefined {
  return catalog.find((recipe) => recipe.outputTypes?.includes("video"))
    ?? (baseRecipe.outputTypes?.includes("video") ? baseRecipe : undefined);
}

function statusLabel(status: string): string {
  switch (status) {
    case "READY": return "就绪";
    case "RUNNING": return "执行中";
    case "WAITING_FOR_SELECTION": return "等待选图";
    case "SUCCEEDED": return "已完成";
    case "PARTIAL_FAILED": return "部分失败";
    case "FAILED": return "失败";
    case "CANCELLED": return "已取消";
    default: return status;
  }
}

function stageLabel(stageType: string): string {
  switch (stageType) {
    case "KREA2_IMAGE_GENERATION": return "Krea2 图片生成";
    case "ASSET_SELECTION": return "人工选图";
    case "H3_VIDEO_GENERATION": return "MiniMax H3 视频生成";
    default: return stageType;
  }
}

export function ProductionRunPanel({ projectId, catalog, baseRecipe, baseValues, onOpenTask, onAdmissionChanged }: Props) {
  const h3Recipe = useMemo(() => h3RecipeFor(catalog, baseRecipe), [baseRecipe, catalog]);
  const promptKey = h3PromptKey(h3Recipe);
  const [name, setName] = useState("Production Run");
  const [imageCount, setImageCount] = useState(2);
  const [h3Prompt, setH3Prompt] = useState("");
  const [h3Profile, setH3Profile] = useState("H3_FAST");
  const [h3Values, setH3Values] = useState<GenerationValues>(() => h3Recipe ? defaultGenerationValues(h3Recipe) : {});
  const [runs, setRuns] = useState<ProductionRun[]>([]);
  const [selectedRun, setSelectedRun] = useState<ProductionRun>();
  const [assets, setAssets] = useState<AssetView[]>([]);
  const [selectedAssetIds, setSelectedAssetIds] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();

  useEffect(() => {
    if (!h3Recipe) return;
    setH3Values(defaultGenerationValues(h3Recipe));
  }, [h3Recipe]);

  useEffect(() => {
    let active = true;
    void listProductionRuns(projectId, 10)
      .then(async (items) => {
        const details = await Promise.all(items.slice(0, 5).map((item) => getProductionRun(projectId, item.id)));
        if (active) {
          setRuns(details);
          if (details[0]) setSelectedRun(details[0]);
        }
      })
      .catch((loadError: unknown) => { if (active) setError(toUserMessage(loadError)); });
    return () => { active = false; };
  }, [projectId]);

  useEffect(() => {
    let active = true;
    void assetLibraryPage({
      projectId,
      category: "ALL",
      mediaType: "IMAGE",
      sourceKind: "ALL",
      createdOrder: "NEWEST",
      limit: 100,
    }).then((page) => { if (active) setAssets(page.items); }).catch(() => undefined);
    return () => { active = false; };
  }, [projectId]);

  function updatePrompt(value: string) {
    setH3Prompt(value);
    if (promptKey) setH3Values((current) => ({ ...current, [promptKey]: { type: "string", value } }));
  }

  async function reload(runId: string) {
    const updated = await refreshProductionRun(projectId, runId);
    setSelectedRun(updated);
    setRuns((current) => current.map((run) => run.id === updated.id ? updated : run));
    return updated;
  }

  async function createRun() {
    if (!h3Recipe) {
      setError("当前目录没有可用的视频 Recipe。 ");
      return;
    }
    setBusy(true); setError(undefined); setNotice(undefined);
    try {
      const created = await createProductionRun({
        projectId,
        name: name.trim() || "Production Run",
        krea2WorkflowVersionId: baseRecipe.workflowVersionId,
        krea2RecipeId: baseRecipe.recipeId,
        krea2Values: baseValues,
        imageCount,
        h3WorkflowVersionId: h3Recipe.workflowVersionId,
        h3RecipeId: h3Recipe.recipeId,
        h3Profile,
        h3Values,
      });
      setSelectedRun(created);
      setRuns((current) => [created, ...current.filter((run) => run.id !== created.id)]);
      setNotice("Production Run 已创建，输入已冻结。 ");
    } catch (createError: unknown) { setError(toUserMessage(createError)); }
    finally { setBusy(false); }
  }

  async function execute(action: () => Promise<ProductionRun>, message: string) {
    setBusy(true); setError(undefined); setNotice(undefined);
    try {
      const updated = await action();
      setSelectedRun(updated);
      setRuns((current) => current.map((run) => run.id === updated.id ? updated : run));
      await onAdmissionChanged?.();
      setNotice(message);
    } catch (actionError: unknown) { setError(toUserMessage(actionError)); }
    finally { setBusy(false); }
  }

  const selectionStage = selectedRun?.stages.find((stage) => stage.stageType === "ASSET_SELECTION");
  const imageStage = selectedRun?.stages.find((stage) => stage.stageType === "KREA2_IMAGE_GENERATION");
  const h3Stage = selectedRun?.stages.find((stage) => stage.stageType === "H3_VIDEO_GENERATION");
  const generatedIds = imageStage?.items.map((item) => item.assetId).filter((id): id is string => Boolean(id)) ?? [];
  const generatedAssets = assets.filter((asset) => generatedIds.includes(asset.id));

  return (
    <section className="production-run-panel" aria-label="Production Runs">
      <div className="production-run-heading">
        <div><span className="section-label">Production Runs</span><h3>Prompt → Krea2 → 选图 → H3</h3><p>固定三阶段生产链，所有生成复用普通 Production Queue，阶段输入创建后冻结。</p></div>
        <span className="workflow-benchmark-badge">Orchestrator Foundation</span>
      </div>
      <div className="production-run-create-grid">
        <label><span>Run 名称</span><input value={name} maxLength={120} onChange={(event) => setName(event.target.value)} /></label>
        <label><span>Krea2 图片数量</span><input type="number" min={1} max={100} value={imageCount} onChange={(event) => setImageCount(Math.max(1, Math.min(100, Number(event.target.value) || 1)))} /></label>
        <label><span>H3 Profile</span><select value={h3Profile} onChange={(event) => setH3Profile(event.target.value)}><option value="H3_FAST">FAST</option><option value="H3_QUALITY">QUALITY</option></select></label>
        <label className="production-run-prompt"><span>H3 Prompt</span><textarea rows={2} value={h3Prompt} onChange={(event) => updatePrompt(event.target.value)} placeholder="输入视频 Prompt" /></label>
      </div>
      <div className="production-run-actions">
        <button type="button" onClick={() => void createRun()} disabled={busy || !h3Recipe}>新建 Production Run</button>
        {selectedRun && <button type="button" className="quiet-button" onClick={() => void execute(() => reload(selectedRun.id), "Production Run 已刷新。 ")} disabled={busy}>刷新</button>}
      </div>
      {error && <p className="error-message" role="alert">{error}</p>}
      {notice && <p className="studio-notice" role="status">{notice}</p>}
      {runs.length > 0 && <div className="production-run-history" aria-label="Production Run 历史">
        {runs.map((run) => <button type="button" key={run.id} className={selectedRun?.id === run.id ? "production-run-history-row production-run-history-row-active" : "production-run-history-row"} onClick={() => { setSelectedRun(run); setSelectedAssetIds([]); }}>{run.name}<span>{statusLabel(run.status)}</span></button>)}
      </div>}
      {selectedRun && <div className="production-run-stages">
        <div className="production-run-status"><strong>{selectedRun.name}</strong><span>{statusLabel(selectedRun.status)}</span><small>{selectedRun.id}</small></div>
        {selectedRun.stages.map((stage) => <article className="production-run-stage" key={stage.id}>
          <div className="production-run-stage-heading"><strong>{stage.ordinal + 1}. {stageLabel(stage.stageType)}</strong><span>{statusLabel(stage.status)}</span><small>{stage.productionBatchId ?? "尚未创建批次"}</small></div>
          {stage.items.map((item) => <div className="production-run-stage-item" key={item.id}><span>#{item.ordinal + 1}</span><b>{statusLabel(item.status)}</b><small>{item.assetId ?? item.taskId ?? "等待队列"}</small>{item.taskId && onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(item.taskId!)}>任务</button>}</div>)}
        </article>)}
        {generatedAssets.length > 0 && <div className="production-run-asset-selection"><strong>选择 Krea2 图片（可多选，顺序即 H3 reference_index）</strong><div className="production-run-asset-grid">{generatedAssets.map((asset) => { const active = selectedAssetIds.includes(asset.id); return <button type="button" key={asset.id} className={active ? "production-run-asset production-run-asset-active" : "production-run-asset"} onClick={() => setSelectedAssetIds((current) => active ? current.filter((id) => id !== asset.id) : [...current, asset.id])}><span>{active ? "✓" : "＋"}</span><strong>{asset.name}</strong><small>{asset.width ?? "—"} × {asset.height ?? "—"}</small></button>; })}</div><button type="button" onClick={() => void execute(() => selectProductionRunAssets(projectId, selectedRun.id, selectedAssetIds), "选图已冻结，H3 Stage 已就绪。 ")} disabled={busy || selectedAssetIds.length === 0 || selectionStage?.status === "SUCCEEDED"}>确认选图</button></div>}
        <div className="production-run-actions">
          {imageStage?.status === "READY" && <button type="button" onClick={() => void execute(() => runProductionImages(projectId, selectedRun.id), "Krea2 图片 Stage 已进入普通串行队列。 ")} disabled={busy}>Run Images</button>}
          {h3Stage?.status === "READY" && <button type="button" onClick={() => void execute(() => runProductionVideo(projectId, selectedRun.id), "H3 视频 Stage 已进入普通串行队列。 ")} disabled={busy}>Run Video</button>}
          {h3Stage?.status === "FAILED" && <button type="button" onClick={() => void execute(() => retryProductionVideo(projectId, selectedRun.id), "H3 已创建新 attempt，Krea2 图片保留。 ")} disabled={busy}>Retry H3</button>}
          {!['SUCCEEDED', 'CANCELLED'].includes(selectedRun.status) && <button type="button" className="quiet-button" onClick={() => void execute(() => cancelProductionRun(projectId, selectedRun.id), "Production Run 已取消，成功资产保留。 ")} disabled={busy}>取消 Run</button>}
        </div>
        {h3Stage?.items.some((item) => item.assetId) && <p className="studio-notice">Final Video Asset：{h3Stage.items.filter((item) => item.assetId).map((item) => item.assetId).join(", ")}</p>}
      </div>}
    </section>
  );
}
