import { useEffect, useMemo, useRef, useState } from "react";
import {
  assetLibraryPage,
  cancelProductionRun,
  createProductionRun,
  getAsset,
  getAssetMediaUrl,
  getProductionRun,
  listProductionRuns,
  listProductionRunTemplates,
  refreshProductionRun,
  retryProductionVideo,
  runProductionImages,
  runProductionVideo,
  saveProductionRunTemplate,
  selectProductionRunAssets,
} from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { GenerationValues, RecipeField, RecipeViewModel } from "../../types/generation";
import type { ProductionRun, ProductionRunStage, ProductionRunTemplate } from "../../types/productionRun";
import { toUserMessage } from "../../i18n/errorMessages";
import { defaultGenerationValues } from "../../stores/studioStore";
import { validateRecipeValues } from "../studio/DynamicFormRenderer";
import {
  h3FamilyForWorkflowId,
  h3RecipeForMode,
  type H3QualityProfile,
} from "../runtime/productRuntimeScope";
import { AssetCard } from "../assets/AssetCard";

type NumericRecipeField = Extract<RecipeField, { type: "integer" | "number" }>;

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  baseRecipe: RecipeViewModel;
  baseValues: GenerationValues;
  onOpenTask?: (taskId: string) => void;
  onAdmissionChanged?: () => Promise<void>;
}

export type ProductionRunVideoMode = "I2V" | "REF2VA";

export const PRODUCTION_RUN_VIDEO_MODES: ReadonlyArray<{
  id: ProductionRunVideoMode;
  label: string;
  recipeMode: "FL2VA_IMAGE_TO_VIDEO" | "REF2VA_IMAGE";
  description: string;
}> = [
  {
    id: "I2V",
    label: "I2V · 单首帧",
    recipeMode: "FL2VA_IMAGE_TO_VIDEO",
    description: "按当前顺序第 1 张图片作为视频首帧。",
  },
  {
    id: "REF2VA",
    label: "REF2VA · 2–N 参考图",
    recipeMode: "REF2VA_IMAGE",
    description: "至少选择 2 张图片，并按顺序写入参考图槽位。",
  },
];

function firstField(recipe: RecipeViewModel | undefined, predicate: (field: RecipeField) => boolean): RecipeField | undefined {
  return recipe?.fields.find(predicate);
}

function h3PromptKey(recipe: RecipeViewModel | undefined): string | undefined {
  return firstField(recipe, (field) => field.type === "textarea" && /prompt/i.test(field.key))?.key
    ?? firstField(recipe, (field) => field.type === "textarea")?.key;
}

type H3Profile = "H3_FAST" | "H3_QUALITY";

function h3ProfileValue(value: string | undefined): H3Profile {
  return value === "H3_QUALITY" ? "H3_QUALITY" : "H3_FAST";
}

function runtimeProfileForH3Profile(profile: H3Profile): H3QualityProfile {
  return profile === "H3_QUALITY" ? "QUALITY" : "FAST";
}

function modeConfig(mode: ProductionRunVideoMode) {
  return PRODUCTION_RUN_VIDEO_MODES.find((option) => option.id === mode) ?? PRODUCTION_RUN_VIDEO_MODES[0];
}

export function h3ReferenceImageMax(recipe: RecipeViewModel | undefined): number {
  const field = recipe?.fields.find((candidate): candidate is Extract<RecipeField, { type: "images" }> => (
    (candidate.key === "reference_images" || candidate.key === "reference_image")
      && candidate.type === "images"
  ));
  return field?.maxItems ?? 9;
}

function h3ReferenceImageMin(recipe: RecipeViewModel | undefined): number {
  const field = recipe?.fields.find((candidate): candidate is Extract<RecipeField, { type: "images" }> => (
    (candidate.key === "reference_images" || candidate.key === "reference_image")
      && candidate.type === "images"
  ));
  return Math.max(2, field?.minItems ?? 0);
}

export function productionRunSelectionBounds(
  mode: ProductionRunVideoMode,
  recipe: RecipeViewModel | undefined,
): { min: number; max: number } {
  return mode === "I2V"
    ? { min: 1, max: 1 }
    : { min: h3ReferenceImageMin(recipe), max: h3ReferenceImageMax(recipe) };
}

export function productionRunSelectionError(
  mode: ProductionRunVideoMode,
  count: number,
  max: number,
  missingCount = 0,
  assetIds: string[] = [],
  min = 2,
): string | undefined {
  if (missingCount > 0) return "部分参考图片尚未加载，请刷新资产库后再确认。";
  if (new Set(assetIds).size !== assetIds.length) return "参考图片不能重复选择。";
  if (mode === "I2V" && count !== 1) return "I2V 需要选择 1 张图片作为首帧。";
  if (mode === "REF2VA" && count < min) return `REF2VA 至少需要选择 ${min} 张图片（当前 ${count} 张）。`;
  if (count > max) return `当前 Recipe 最多支持 ${max} 张参考图片。`;
  return undefined;
}

export function moveProductionRunAsset(assetIds: string[], assetId: string, delta: number): string[] {
  const index = assetIds.indexOf(assetId);
  const target = index + delta;
  if (index < 0 || target < 0 || target >= assetIds.length) return assetIds;
  const next = [...assetIds];
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}

export function productionRunSelectionIds(selectionStage: ProductionRunStage | undefined): string[] {
  return [...(selectionStage?.items ?? [])]
    .filter((item) => Boolean(item.sourceAssetId ?? item.assetId))
    .sort((left, right) => (
      (left.referenceIndex ?? left.ordinal) - (right.referenceIndex ?? right.ordinal)
    ))
    .map((item) => item.sourceAssetId ?? item.assetId)
    .filter((assetId): assetId is string => Boolean(assetId));
}

function productionRunGeneratedAssetIds(run: ProductionRun | undefined): string[] {
  const imageStage = run?.stages.find((stage) => stage.stageType === "KREA2_IMAGE_GENERATION");
  return [...(imageStage?.items ?? [])]
    .sort((left, right) => left.ordinal - right.ordinal)
    .map((item) => item.assetId)
    .filter((assetId): assetId is string => Boolean(assetId));
}

function productionRunModeForRun(run: ProductionRun | undefined, catalog: RecipeViewModel[]): ProductionRunVideoMode | undefined {
  const stage = run?.stages.find((candidate) => candidate.stageType === "H3_VIDEO_GENERATION");
  const recipe = catalog.find((candidate) => (
    candidate.recipeId === stage?.recipeId && candidate.workflowVersionId === stage?.workflowVersionId
  ));
  if (!recipe) return undefined;
  const family = h3FamilyForWorkflowId(recipe.workflowId);
  return family === "REF2VA" ? "REF2VA" : family === "FL2VA" ? "I2V" : undefined;
}

function draftValueMatchesField(value: GenerationValues[string], field: RecipeField): boolean {
  if (!value) return false;
  switch (field.type) {
    case "textarea": return value.type === "string";
    case "integer": return value.type === "integer";
    case "number": return value.type === "number";
    case "seed": return value.type === "seed_random" || value.type === "seed_fixed";
    case "image": return value.type === "image_asset";
    case "images": return value.type === "image_assets";
    case "video": return value.type === "video_asset";
    case "audio": return value.type === "audio_asset";
    case "videos": return value.type === "video_assets";
    case "audios": return value.type === "audio_assets";
  }
}

function preserveH3Values(current: GenerationValues, recipe: RecipeViewModel): GenerationValues {
  const next = defaultGenerationValues(recipe);
  recipe.fields.forEach((field) => {
    const value = current[field.key];
    if (draftValueMatchesField(value, field)) next[field.key] = value;
  });
  return next;
}

function normalizedImageCount(value: number, mode: ProductionRunVideoMode): number {
  const fallback = mode === "REF2VA" ? 2 : 1;
  const minimum = mode === "REF2VA" ? 2 : 1;
  return Math.max(minimum, Math.min(100, Number.isFinite(value) ? value : fallback));
}

function recipeValueError(recipe: RecipeViewModel, values: GenerationValues): string | undefined {
  const [key, message] = Object.entries(validateRecipeValues(recipe, values))[0] ?? [];
  return key && message ? `${key}：${message}` : undefined;
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

function FinalVideoPreview({ projectId, assetIds }: { projectId: string; assetIds: string[] }) {
  const [failedAssetIds, setFailedAssetIds] = useState<Set<string>>(new Set());

  useEffect(() => {
    setFailedAssetIds(new Set());
  }, [projectId, assetIds.join("|")]);

  if (!assetIds.length) return null;
  return (
    <div className="production-run-final-video" aria-label="最终视频预览">
      <strong>Final Video</strong>
      {assetIds.map((assetId) => failedAssetIds.has(assetId) ? (
        <p className="error-message" role="alert" key={assetId}>视频预览加载失败：{assetId}。可刷新 Run 或查看任务诊断。</p>
      ) : (
        <div className="production-run-final-video-item" key={assetId}>
          <video
            src={getAssetMediaUrl(projectId, assetId, "video")}
            controls
            preload="metadata"
            playsInline
            aria-label={`最终视频 ${assetId}`}
            onError={() => setFailedAssetIds((current) => new Set([...current, assetId]))}
          />
          <small>{assetId}</small>
        </div>
      ))}
    </div>
  );
}

export function ProductionRunPanel({ projectId, catalog, baseRecipe, baseValues, onOpenTask, onAdmissionChanged }: Props) {
  const [name, setName] = useState("Production Run");
  const [imageCount, setImageCount] = useState(2);
  const [videoMode, setVideoMode] = useState<ProductionRunVideoMode>("I2V");
  const [h3Prompt, setH3Prompt] = useState("");
  const [h3Profile, setH3Profile] = useState<H3Profile>("H3_FAST");
  const selectedMode = modeConfig(videoMode);
  const h3Recipe = useMemo(
    () => h3RecipeForMode(catalog, selectedMode.recipeMode, runtimeProfileForH3Profile(h3Profile)),
    [catalog, h3Profile, selectedMode.recipeMode],
  );
  const numericFields = useMemo(() => h3NumericFields(h3Recipe), [h3Recipe]);
  const promptKey = h3PromptKey(h3Recipe);
  const [h3Values, setH3Values] = useState<GenerationValues>(() => h3Recipe ? defaultGenerationValues(h3Recipe) : {});
  const [runs, setRuns] = useState<ProductionRun[]>([]);
  const [templates, setTemplates] = useState<ProductionRunTemplate[]>([]);
  const [selectedTemplateId, setSelectedTemplateId] = useState("");
  const [selectedRun, setSelectedRun] = useState<ProductionRun>();
  const [assets, setAssets] = useState<AssetView[]>([]);
  const [selectedAssetIds, setSelectedAssetIds] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [runsLoading, setRunsLoading] = useState(true);
  const [assetsLoading, setAssetsLoading] = useState(false);
  const [assetError, setAssetError] = useState<string>();
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const assetsRequestRef = useRef(0);

  useEffect(() => {
    if (!h3Recipe) return;
    setH3Values((current) => preserveH3Values(current, h3Recipe));
  }, [h3Recipe, h3Prompt]);

  useEffect(() => {
    if (!h3Prompt && promptKey) {
      const value = h3Values[promptKey];
      if (value?.type === "string" && value.value) setH3Prompt(value.value);
    }
  }, [h3Prompt, h3Values, promptKey]);

  function adoptRun(run: ProductionRun) {
    setSelectedRun(run);
    const persistedSelection = productionRunSelectionIds(
      run.stages.find((stage) => stage.stageType === "ASSET_SELECTION"),
    );
    setSelectedAssetIds(persistedSelection);
    const persistedMode = productionRunModeForRun(run, catalog);
    if (persistedMode) setVideoMode(persistedMode);
  }

  useEffect(() => {
    let active = true;
    void listProductionRuns(projectId, 10)
      .then(async (items) => {
        const details = await Promise.all(items.slice(0, 5).map((item) => getProductionRun(projectId, item.id)));
        if (active) {
          setRuns(details);
          if (details[0]) adoptRun(details[0]);
        }
      })
      .catch((loadError: unknown) => { if (active) setError(toUserMessage(loadError)); })
      .finally(() => { if (active) setRunsLoading(false); });
    return () => { active = false; };
  }, [catalog, projectId]);

  useEffect(() => {
    let active = true;
    void listProductionRunTemplates(projectId)
      .then((items) => { if (active) setTemplates(items); })
      .catch(() => undefined);
    return () => { active = false; };
  }, [projectId]);

  async function refreshAssets(candidateIds: string[] = []) {
    const requestId = assetsRequestRef.current + 1;
    assetsRequestRef.current = requestId;
    setAssetsLoading(true);
    setAssetError(undefined);
    try {
      const page = await assetLibraryPage({
        projectId,
        category: "ALL",
        mediaType: "IMAGE",
        sourceKind: "ALL",
        createdOrder: "NEWEST",
        limit: 100,
      });
      const pageIds = new Set(page.items.map((asset) => asset.id));
      const missing = candidateIds.filter((assetId) => !pageIds.has(assetId));
      const loaded = await Promise.all(missing.map((assetId) => getAsset(projectId, assetId).catch(() => undefined)));
      const byId = new Map(page.items.map((asset) => [asset.id, asset]));
      loaded.filter((asset): asset is AssetView => Boolean(asset)).forEach((asset) => byId.set(asset.id, asset));
      if (requestId !== assetsRequestRef.current) return;
      setAssets((current) => {
        const merged = new Map(current.map((asset) => [asset.id, asset]));
        byId.forEach((asset, assetId) => merged.set(assetId, asset));
        return [...merged.values()];
      });
    } catch (loadError: unknown) {
      const message = toUserMessage(loadError);
      if (requestId === assetsRequestRef.current) setAssetError(message);
      throw loadError;
    } finally {
      if (requestId === assetsRequestRef.current) setAssetsLoading(false);
    }
  }

  useEffect(() => {
    setAssets([]);
    void refreshAssets().catch(() => undefined);
  }, [projectId]);

  const selectedRunGeneratedIds = useMemo(() => productionRunGeneratedAssetIds(selectedRun), [selectedRun]);
  const generatedAssetRefreshKey = selectedRunGeneratedIds.join("|");
  useEffect(() => {
    if (!selectedRunGeneratedIds.length) return;
    void refreshAssets(selectedRunGeneratedIds).catch(() => undefined);
  }, [generatedAssetRefreshKey, projectId]);

  function updatePrompt(value: string) {
    setH3Prompt(value);
    if (promptKey) setH3Values((current) => ({ ...current, [promptKey]: { type: "string", value } }));
  }

  function applyTemplate(template: ProductionRunTemplate) {
    setSelectedTemplateId(template.id);
    setName(template.name);
    setImageCount(normalizedImageCount(template.defaultImageCount, videoMode));
    setH3Profile(h3ProfileValue(template.h3Profile));
    setH3Values((current) => {
      const next = { ...current };
      numericFields.forEach((field) => {
        const descriptor = `${field.key} ${field.label}`;
        const value = /duration/i.test(descriptor)
          ? template.defaultDurationSeconds
          : /width/i.test(descriptor)
            ? template.defaultWidth
            : /height/i.test(descriptor)
              ? template.defaultHeight
              : undefined;
        if (value !== undefined) next[field.key] = { type: field.type, value };
      });
      return next;
    });
    setNotice(`已加载模板：${template.name}`);
  }

  function changeVideoMode(nextMode: ProductionRunVideoMode) {
    if (nextMode === videoMode) return;
    const currentIds = selectedAssetIds;
    setVideoMode(nextMode);
    setError(undefined);
    if (nextMode === "REF2VA") {
      setNotice(currentIds.length < 2
        ? "已保留当前选图；REF2VA 至少需要 2 张参考图片。"
        : "已保留当前参考图顺序，可继续调整 @图片1、@图片2…");
      return;
    }
    if (currentIds.length > 0) {
      setSelectedAssetIds([currentIds[0]]);
      setNotice("已将当前顺序第 1 张图片作为 I2V 首帧；其余参考图已移出当前选择。 ");
    } else {
      setNotice("I2V 将使用当前顺序第 1 张图片作为首帧。 ");
    }
  }

  async function saveTemplate() {
    if (!h3Recipe) return;
    const validationError = recipeValueError(h3Recipe, h3Values);
    if (validationError) {
      setError(`H3 参数无效：${validationError}`);
      return;
    }
    setBusy(true); setError(undefined); setNotice(undefined);
    try {
      const saved = await saveProductionRunTemplate({
        projectId,
        name: name.trim() || "Production Run 模板",
        krea2WorkflowVersionId: baseRecipe.workflowVersionId,
        krea2RecipeId: baseRecipe.recipeId,
        defaultImageCount: imageCount,
        h3WorkflowVersionId: h3Recipe.workflowVersionId,
        h3RecipeId: h3Recipe.recipeId,
        h3Profile,
        defaultDurationSeconds: fieldNumericValue(h3Values, numericFields, /duration/i),
        defaultWidth: fieldNumericValue(h3Values, numericFields, /width/i),
        defaultHeight: fieldNumericValue(h3Values, numericFields, /height/i),
      });
      setTemplates((current) => [saved, ...current.filter((template) => template.id !== saved.id)]);
      setSelectedTemplateId(saved.id);
      setNotice(`模板已保存：${saved.name}`);
    } catch (saveError: unknown) { setError(toUserMessage(saveError)); }
    finally { setBusy(false); }
  }

  async function reload(runId: string) {
    return refreshProductionRun(projectId, runId);
  }

  async function createRun() {
    if (!h3Recipe) {
      setError(`当前 ${selectedMode.label} / ${h3Profile} 没有可用的视频 Recipe。`);
      return;
    }
    const validationError = recipeValueError(h3Recipe, h3Values);
    if (validationError) {
      setError(`H3 参数无效：${validationError}`);
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
      adoptRun(created);
      setRuns((current) => [created, ...current.filter((run) => run.id !== created.id)]);
      setNotice(`${selectedMode.label} Production Run 已创建，输入已冻结。 `);
    } catch (createError: unknown) { setError(toUserMessage(createError)); }
    finally { setBusy(false); }
  }

  async function execute(action: () => Promise<ProductionRun>, message: string) {
    setBusy(true); setError(undefined); setNotice(undefined);
    try {
      const updated = await action();
      adoptRun(updated);
      setRuns((current) => current.map((run) => run.id === updated.id ? updated : run));
      await refreshAssets(productionRunGeneratedAssetIds(updated)).catch(() => undefined);
      try {
        await onAdmissionChanged?.();
      } catch {
        setNotice(`${message}（状态刷新失败，请手动刷新。）`);
        return;
      }
      setNotice(message);
    } catch (actionError: unknown) { setError(toUserMessage(actionError)); }
    finally { setBusy(false); }
  }

  const selectionStage = selectedRun?.stages.find((stage) => stage.stageType === "ASSET_SELECTION");
  const imageStage = selectedRun?.stages.find((stage) => stage.stageType === "KREA2_IMAGE_GENERATION");
  const h3Stage = selectedRun?.stages.find((stage) => stage.stageType === "H3_VIDEO_GENERATION");
  const generatedIds = productionRunGeneratedAssetIds(selectedRun);
  const generatedAssets = generatedIds
    .map((assetId) => assets.find((asset) => asset.id === assetId))
    .filter((asset): asset is AssetView => Boolean(asset));
  const selectedAssetSet = new Set(selectedAssetIds);
  const missingSelectedCount = selectedAssetIds.filter((assetId) => !assets.some((asset) => asset.id === assetId)).length;
  const selectedRunMode = productionRunModeForRun(selectedRun, catalog);
  const selectedRunRecipe = selectedRun && h3Stage
    ? catalog.find((recipe) => (
      recipe.workflowVersionId === h3Stage.workflowVersionId && recipe.recipeId === h3Stage.recipeId
    ))
    : undefined;
  const selectionMode = selectedRunMode ?? videoMode;
  const selectionRecipe = selectedRunRecipe ?? h3Recipe;
  const selectionBounds = productionRunSelectionBounds(selectionMode, selectionRecipe);
  const selectionFrozen = selectionStage?.status === "SUCCEEDED" || Boolean(h3Stage?.productionBatchId);
  const modeMismatch = Boolean(selectedRunMode && selectedRunMode !== videoMode);
  const unsupportedRunMode = Boolean(selectedRun && h3Stage && !selectedRunMode);
  const selectionError = productionRunSelectionError(
    selectionMode,
    selectedAssetIds.length,
    selectionBounds.max,
    selectionFrozen ? 0 : missingSelectedCount,
    selectedAssetIds,
    selectionBounds.min,
  );
  const selectionReady = !selectionError && !modeMismatch && !unsupportedRunMode;
  const failedH3Item = h3Stage?.items.find((item) => item.errorCode || item.errorMessage);
  const finalVideoIds = [...new Set(
    h3Stage?.items
      .filter((item) => item.status === "SUCCEEDED")
      .map((item) => item.assetId)
      .filter((id): id is string => Boolean(id)) ?? [],
  )];

  function toggleAsset(asset: AssetView) {
    if (busy || selectionFrozen || modeMismatch || unsupportedRunMode) return;
    if (selectionMode === "I2V") {
      setSelectedAssetIds((current) => current.length === 1 && current[0] === asset.id ? [] : [asset.id]);
      return;
    }
    if (selectedAssetSet.has(asset.id)) {
      setSelectedAssetIds((current) => current.filter((assetId) => assetId !== asset.id));
      return;
    }
    if (selectedAssetIds.length >= selectionBounds.max) {
      setNotice(`当前 Recipe 最多支持 ${selectionBounds.max} 张参考图片。`);
      return;
    }
    setSelectedAssetIds((current) => [...current, asset.id]);
  }

  function moveSelectedAsset(assetId: string, delta: number) {
    setSelectedAssetIds((current) => moveProductionRunAsset(current, assetId, delta));
  }

  function removeSelectedAsset(assetId: string) {
    setSelectedAssetIds((current) => current.filter((id) => id !== assetId));
  }

  return (
    <section className="production-run-panel" aria-label="Production Runs">
      <div className="production-run-heading">
        <div><span className="section-label">Production Runs</span><h3>Prompt → Krea2 → 选图 → H3</h3><p>固定三阶段生产链；H3 模式在创建 Run 时冻结，选图顺序会写入 reference_index。</p></div>
        <span className="workflow-benchmark-badge">Orchestrator Foundation</span>
      </div>
      <div className="production-run-create-grid">
        <label><span>Run 名称</span><input value={name} maxLength={120} onChange={(event) => setName(event.target.value)} /></label>
        <label><span>Krea2 图片数量</span><input type="number" min={videoMode === "REF2VA" ? 2 : 1} max={100} value={imageCount} onChange={(event) => setImageCount(normalizedImageCount(Number(event.target.value), videoMode))} /></label>
        <label><span>H3 模式</span><select value={videoMode} onChange={(event) => changeVideoMode(event.target.value as ProductionRunVideoMode)} disabled={busy}><option value="I2V">I2V · 单首帧</option><option value="REF2VA">REF2VA · 2–N 参考图</option></select><small>{selectedMode.description}</small></label>
        <label><span>H3 Profile</span><select value={h3Profile} onChange={(event) => setH3Profile(h3ProfileValue(event.target.value))} disabled={busy}><option value="H3_FAST">FAST</option><option value="H3_QUALITY">QUALITY</option></select></label>
        {numericFields.map((field) => <label key={field.key}><span>{field.label}</span><input type="number" min={field.min} max={field.max} step={field.step} value={numericValue(h3Values, field.key) ?? ""} onChange={(event) => { const value = Number(event.target.value); if (!Number.isFinite(value)) return; setH3Values((current) => ({ ...current, [field.key]: { type: field.type, value } })); }} /></label>)}
        <label className="production-run-prompt"><span>H3 Prompt</span><textarea rows={2} value={h3Prompt} onChange={(event) => updatePrompt(event.target.value)} placeholder="输入视频 Prompt" /></label>
      </div>
      <div className="production-run-actions">
        <button type="button" onClick={() => void createRun()} disabled={busy || !h3Recipe}>新建 Production Run</button>
        <button type="button" className="quiet-button" onClick={() => void saveTemplate()} disabled={busy || !h3Recipe}>保存模板</button>
        {templates.length > 0 && <label className="production-run-template-picker"><span>模板</span><select value={selectedTemplateId} onChange={(event) => { const template = templates.find((item) => item.id === event.target.value); if (template) applyTemplate(template); }}><option value="">选择模板</option>{templates.map((template) => <option key={template.id} value={template.id}>{template.name}</option>)}</select></label>}
        {selectedRun && <button type="button" className="quiet-button" onClick={() => void execute(() => reload(selectedRun.id), "Production Run 已刷新。 ")} disabled={busy}>刷新</button>}
      </div>
      {!h3Recipe && <p className="disabled-note" role="status">当前 {selectedMode.label} / {h3Profile} 没有可用的 H3 视频 Recipe，Production Run 暂不可用。</p>}
      {modeMismatch && <p className="disabled-note" role="status">当前 Run 的 H3 模式已冻结为 {selectedRunMode}；切换只影响新建 Run，确认当前选图前请切回 {selectedRunMode}。</p>}
      {unsupportedRunMode && <p className="error-message" role="alert">当前 Run 使用了不受 Production Run 支持的 H3 Recipe，已阻止继续提交。</p>}
      {error && <p className="error-message" role="alert">{error}</p>}
      {notice && <p className="studio-notice" role="status">{notice}</p>}
      {runsLoading && <p className="disabled-note" role="status">正在加载 Production Run 历史…</p>}
      {!runsLoading && runs.length === 0 && <p className="disabled-note" role="status">暂无 Production Run，请先新建一个运行。</p>}
      {!runsLoading && runs.length > 0 && <div className="production-run-history" aria-label="Production Run 历史">
        {runs.map((run) => <button type="button" key={run.id} className={selectedRun?.id === run.id ? "production-run-history-row production-run-history-row-active" : "production-run-history-row"} onClick={() => { adoptRun(run); void refreshAssets(productionRunGeneratedAssetIds(run)).catch(() => undefined); }}>{run.name}<span>{statusLabel(run.status)}</span></button>)}
      </div>}
      {selectedRun && <div className="production-run-stages">
        <div className="production-run-status"><strong>{selectedRun.name}</strong><span>{statusLabel(selectedRun.status)}</span><small>{selectedRun.id}</small></div>
        {imageStage?.status === "SUCCEEDED" && selectionStage?.status !== "SUCCEEDED" && generatedAssets.length === 0 && <p className="disabled-note" role="status">{assetsLoading ? "Krea2 图片已完成，正在加载可选图片…" : "Krea2 图片已完成，但候选图片尚未载入；请刷新资产库。"}</p>}
        {assetError && generatedIds.length > 0 && <p className="error-message" role="alert">候选图片加载失败：{assetError}</p>}
        {imageStage?.status === "PARTIAL_FAILED" && <p className="disabled-note" role="status">部分 Krea2 候选生成失败；已成功的图片仍可继续选图。</p>}
        {h3Stage?.status === "RUNNING" && <p className="disabled-note" role="status">H3 正在生成，按钮已锁定；可点击刷新查看进度。</p>}
        {h3Stage?.status === "FAILED" && <p className="error-message" role="alert">H3 生成失败{failedH3Item?.errorCode ? `（${failedH3Item.errorCode}）` : ""}，可查看诊断后重试。</p>}
        {selectedRun.stages.map((stage) => <article className="production-run-stage" key={stage.id}>
          <div className="production-run-stage-heading"><strong>{stage.ordinal + 1}. {stageLabel(stage.stageType)}</strong><span>{statusLabel(stage.status)}</span><small>{stage.productionBatchId ?? "尚未创建批次"}</small></div>
          {stage.items.map((item) => <div className="production-run-stage-item" key={item.id}><span>#{item.ordinal + 1}</span><b>{statusLabel(item.status)}</b><small>{item.assetId ?? item.taskId ?? "等待队列"}</small>{item.taskId && onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(item.taskId!)}>任务</button>}</div>)}
        </article>)}
        {generatedAssets.length > 0 && <div className="production-run-asset-selection">
          <div className="production-run-selection-heading"><div><strong>{selectionMode === "REF2VA" ? "选择 REF2VA 参考图片" : "选择 I2V 首帧图片"}</strong><small>{selectionMode === "REF2VA" ? `已选 ${selectedAssetIds.length}/${selectionBounds.max} · 至少 ${selectionBounds.min} 张` : "当前顺序第 1 张会作为首帧"}</small></div><span>{selectionFrozen ? "顺序已冻结" : "可调整顺序"}</span></div>
          <div className="production-run-asset-grid">
            {generatedAssets.map((asset) => {
              const selectedIndex = selectedAssetIds.indexOf(asset.id);
              const active = selectedIndex >= 0;
              return <div key={asset.id} className={active ? "production-run-asset production-run-asset-active" : "production-run-asset"}>
                <span className="production-run-asset-order">{active ? `@图片${selectedIndex + 1}` : "可选"}</span>
                <AssetCard projectId={projectId} asset={asset} onSelect={() => toggleAsset(asset)} selectionMode selected={active} disabled={busy || selectionFrozen || modeMismatch || unsupportedRunMode} onToggleSelection={() => toggleAsset(asset)} />
              </div>;
            })}
          </div>
          <div className="production-run-reference-list" aria-label="已选择图片顺序">
            <div className="production-run-reference-heading"><strong>{selectionMode === "REF2VA" ? "REF2VA 图片顺序" : "I2V 首帧"}</strong><span>按列表顺序提交</span></div>
            {selectedAssetIds.map((assetId, index) => {
              const asset = assets.find((candidate) => candidate.id === assetId);
              return <div className="production-run-reference-row" key={assetId}>
                <span className="production-run-reference-index">@图片{index + 1}</span>
                <strong className="production-run-reference-name">{asset?.name ?? "图片暂不可用"}</strong>
                <small>{asset ? `${asset.width ?? "—"} × ${asset.height ?? "—"}` : assetId}</small>
                <button type="button" className="quiet-button" onClick={() => moveSelectedAsset(assetId, -1)} disabled={busy || selectionFrozen || modeMismatch || unsupportedRunMode || index === 0} aria-label={`上移@图片${index + 1}`}>↑</button>
                <button type="button" className="quiet-button" onClick={() => moveSelectedAsset(assetId, 1)} disabled={busy || selectionFrozen || modeMismatch || unsupportedRunMode || index === selectedAssetIds.length - 1} aria-label={`下移@图片${index + 1}`}>↓</button>
                <button type="button" className="quiet-button" onClick={() => removeSelectedAsset(assetId)} disabled={busy || selectionFrozen || modeMismatch || unsupportedRunMode} aria-label={`移除@图片${index + 1}`}>移除</button>
              </div>;
            })}
            {!selectedAssetIds.length && <span className="production-run-reference-empty">尚未选择图片。</span>}
          </div>
          {selectionError && <p className="error-message" role="alert">{selectionError}</p>}
          {selectionFrozen && missingSelectedCount > 0 && <p className="disabled-note" role="status">已冻结的参考图片中有素材当前不可预览；顺序仍按持久化记录保留。</p>}
          <button type="button" onClick={() => void execute(() => selectProductionRunAssets(projectId, selectedRun.id, selectedAssetIds), "选图已冻结，H3 Stage 已就绪。 ")} disabled={busy || selectionFrozen || !selectionReady || modeMismatch}>确认选图</button>
        </div>}
        <div className="production-run-actions">
          {imageStage?.status === "READY" && <button type="button" onClick={() => void execute(() => runProductionImages(projectId, selectedRun.id), "Krea2 图片 Stage 已进入普通串行队列。 ")} disabled={busy}>Run Images</button>}
          {h3Stage?.status === "READY" && <button type="button" onClick={() => void execute(() => runProductionVideo(projectId, selectedRun.id), "H3 视频 Stage 已进入普通串行队列。 ")} disabled={busy || !selectionReady || modeMismatch}>Run Video</button>}
          {h3Stage?.status === "FAILED" && <button type="button" onClick={() => void execute(() => retryProductionVideo(projectId, selectedRun.id), "H3 已创建新 attempt，Krea2 图片保留。 ")} disabled={busy}>Retry H3</button>}
          {!['SUCCEEDED', 'CANCELLED'].includes(selectedRun.status) && <button type="button" className="quiet-button" onClick={() => void execute(() => cancelProductionRun(projectId, selectedRun.id), "Production Run 已取消，成功资产保留。 ")} disabled={busy}>取消 Run</button>}
        </div>
        {finalVideoIds.length > 0 && <FinalVideoPreview projectId={projectId} assetIds={finalVideoIds} />}
        {h3Stage?.status === "SUCCEEDED" && finalVideoIds.length === 0 && <p className="disabled-note" role="status">H3 已完成，但当前没有可播放的视频资产；请刷新 Run 或查看诊断。</p>}
        <details className="production-run-diagnostics">
          <summary>诊断 / 冻结配置</summary>
          <p>Run ID：{selectedRun.id} · 当前阶段：{selectedRun.currentStageOrdinal + 1}</p>
          {selectedRun.stages.map((stage) => <div key={stage.id}>
            <strong>{stage.ordinal + 1}. {stageLabel(stage.stageType)}</strong>
            <p>Stage ID：{stage.id} · Workflow Version：{stage.workflowVersionId ?? "—"} · Recipe：{stage.recipeId ?? "—"} · Batch：{stage.productionBatchId ?? "—"}</p>
            <pre>{JSON.stringify(stage.frozenConfig, null, 2) ?? "{}"}</pre>
            {stage.items.map((item) => <p key={item.id}>Item {item.ordinal + 1} · attempt {item.attempt} · task {item.taskId ?? "—"} · asset {item.assetId ?? "—"} · source {item.sourceAssetId ?? "—"} · reference_index {item.referenceIndex ?? "—"} · submission {item.submissionIdempotencyKey ?? "—"}{item.errorCode ? ` · ${item.errorCode}: ${item.errorMessage ?? ""}` : ""}</p>)}
          </div>)}
        </details>
      </div>}
    </section>
  );
}

function h3NumericFields(recipe: RecipeViewModel | undefined): NumericRecipeField[] {
  return recipe?.fields.filter((field): field is NumericRecipeField =>
    (field.type === "integer" || field.type === "number")
    && /(duration|width|height|resolution)/i.test(`${field.key} ${field.label}`),
  ).slice(0, 3) ?? [];
}

function numericValue(values: GenerationValues, key: string): number | undefined {
  const draft = values[key];
  return draft && (draft.type === "integer" || draft.type === "number") ? draft.value : undefined;
}

function fieldNumericValue(
  values: GenerationValues,
  fields: RecipeField[],
  hint: RegExp,
): number | undefined {
  const field = fields.find((candidate) => hint.test(`${candidate.key} ${candidate.label}`));
  return field ? numericValue(values, field.key) : undefined;
}
