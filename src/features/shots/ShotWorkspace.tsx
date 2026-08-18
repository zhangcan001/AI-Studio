import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  createShot,
  bulkAssignShotPrompt,
  bulkSetShotStageConfig,
  deleteShot,
  exportProjectManifest,
  generateShot,
  getAsset,
  getShot,
  getProductionBatchRunbook,
  getSeriesProductionPlan,
  listPromptLibrary,
  listProductionStructure,
  listReferenceAnchors,
  listBatchWorkflowPresets,
  listRecentAssets,
  listShots,
  readAssetImage,
  readAssetThumbnail,
  requeueProductionQueueItemByItem,
  replaceShotReferences,
  reorderShots,
  selectShotResult,
  prepareSeriesProduction,
  previewPromptTemplateBulk,
  applyPromptTemplate,
  setShotStageConfig,
  startProductionQueue,
  updateShot,
} from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { DraftValue, RecipeField, RecipeViewModel } from "../../types/generation";
import type { PromptEntryView } from "../../types/prompt";
import type { ReferenceAnchorView } from "../../types/referenceAnchor";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ProductionBatchRunbookView } from "../../types/productionBatchRunbook";
import type {
  SeriesPromptBulkRequest,
  SeriesPresetApplyRequest,
} from "../../types/seriesProduction";
import type { BatchWorkflowPreset } from "../../types/sceneProduction";
import type { ShotInputValues, ShotStage, ShotView } from "../../types/shot";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime } from "../../i18n/statusLabels";
import { deriveShotStatus, recentShotFailure, shotStatusLabels } from "./shotDomain";
import { ShotBatchReviewBoard } from "./ShotBatchReviewBoard";
import { ProjectProductionPipeline } from "./ProjectProductionPipeline";
import { ShotBulkImportPanel } from "./ShotBulkImportPanel";
import { ShotListToolbar } from "./ShotListToolbar";
import { ProductionStructurePanel } from "./ProductionStructurePanel";
import { PromptTemplatePanel } from "./PromptTemplatePanel";
import { SceneProductionPanel } from "./SceneProductionPanel";
import { EpisodeProductionPanel } from "./EpisodeProductionPanel";
import { SeriesProductionPanel } from "./SeriesProductionPanel";
import { ProductionBatchRunbookPanel } from "../production/ProductionBatchRunbookPanel";
import {
  appendAnchorReferences,
  replaceWithAnchorReferences,
} from "./referenceAnchorApply";
import {
  buildShotListView,
  defaultShotListControls,
  isShotListReorderDisabled,
  updateShotListControls,
  type ShotListControls,
} from "./shotListQuery";
import { EMPTY_PRODUCTION_STRUCTURE, findProductionSceneParent, productionSceneOptions, shotSceneIndex } from "./productionStructureState";
import { isPromptTemplateText } from "../prompts/promptTemplateState";
import {
  filterProductionRuntimeCatalog,
  h3FamilyForWorkflowId,
  h3QualityProfileForWorkflowId,
  isProductionRuntimeForStage,
  MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
} from "../runtime/productRuntimeScope";
import "./ShotWorkspace.css";

interface Props {
  projectId: string;
  projectName?: string;
  projectDescription?: string | null;
  catalog: RecipeViewModel[];
  initialSelectedShotId?: string;
  onShotSelected?: (shotId?: string) => void;
  onOpenInStudio: (shot: ShotView, stage: ShotStage, recipe: RecipeViewModel) => void;
  onOpenTask?: (taskId: string) => void;
  onOpenProductionQueue?: () => void;
}

type StageDraft = {
  workflowVersionId: string;
  recipeId: string;
  values: ShotInputValues;
};

const emptyStageDrafts: Partial<Record<ShotStage, StageDraft>> = {};

export function ShotWorkspace({ projectId, projectName, projectDescription, catalog, initialSelectedShotId, onShotSelected, onOpenInStudio, onOpenTask, onOpenProductionQueue }: Props) {
  const [shots, setShots] = useState<ShotView[]>([]);
  const [selectedShotId, setSelectedShotId] = useState<string | undefined>(initialSelectedShotId);
  const [stage, setStage] = useState<ShotStage>("image");
  const [stageDrafts, setStageDrafts] = useState<Partial<Record<ShotStage, StageDraft>>>(emptyStageDrafts);
  const [dirtyStages, setDirtyStages] = useState<Set<ShotStage>>(new Set());
  const [references, setReferences] = useState<Record<ShotStage, string[]>>({ image: [], video: [] });
  const [assets, setAssets] = useState<AssetView[]>([]);
  const [referenceAnchors, setReferenceAnchors] = useState<ReferenceAnchorView[]>([]);
  const [productionStructure, setProductionStructure] = useState<ProductionStructureTree>(() => EMPTY_PRODUCTION_STRUCTURE(projectId));
  const [productionBatchRunbook, setProductionBatchRunbook] = useState<ProductionBatchRunbookView>(() => emptyRunbook(projectId));
  const [batchWorkflowPresets, setBatchWorkflowPresets] = useState<BatchWorkflowPreset[]>([]);
  const [selectedAnchorId, setSelectedAnchorId] = useState("");
  const [promptEntries, setPromptEntries] = useState<PromptEntryView[]>([]);
  const [selectedPromptId, setSelectedPromptId] = useState("");
  const [name, setName] = useState("");
  const [promptText, setPromptText] = useState("");
  const [promptProvenance, setPromptProvenance] = useState<{ entryId: string; versionId: string }>();
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [bulkImportOpen, setBulkImportOpen] = useState(false);
  const [shotListControls, setShotListControls] = useState<ShotListControls>(defaultShotListControls);
  const reloadGeneration = useRef(0);

  const selectedShot = shots.find((shot) => shot.id === selectedShotId);
  const shotSceneIds = useMemo(() => shotSceneIndex(productionStructure), [productionStructure]);
  const sceneFilterOptions = useMemo(() => productionSceneOptions(productionStructure), [productionStructure]);
  const selectedPromptEntry = promptEntries.find((entry) => entry.id === selectedPromptId);
  const selectedPromptVersion = selectedPromptEntry?.versions.find((version) => version.id === selectedShot?.promptVersionId)
    ?? selectedPromptEntry?.versions[selectedPromptEntry.versions.length - 1];
  const selectedSceneContext = selectedShot?.id && shotSceneIds[selectedShot.id]
    ? findProductionSceneParent(productionStructure, shotSceneIds[selectedShot.id])
    : undefined;
  const shotList = useMemo(() => buildShotListView(shots, shotListControls, shotSceneIds), [shots, shotListControls, shotSceneIds]);
  const currentDraft = stageDrafts[stage];
  const productCatalog = useMemo(() => filterProductionRuntimeCatalog(catalog), [catalog]);
  const stageRecipes = useMemo(
    () => productCatalog.filter((recipe) =>
      isProductionRuntimeForStage(stage, recipe.workflowId) && (recipe.outputTypes?.includes(stage) ?? false),
    ),
    [productCatalog, stage],
  );
  const currentRecipe = productCatalog.find(
    (recipe) =>
      recipe.workflowVersionId === currentDraft?.workflowVersionId &&
      recipe.recipeId === currentDraft?.recipeId &&
      isProductionRuntimeForStage(stage, recipe.workflowId),
  );
  const currentReferences = references[stage] ?? [];
  const selectedAnchor = referenceAnchors.find((anchor) => anchor.id === selectedAnchorId);
  const ref2vaMode = stage === "video" && isRef2vaRecipe(currentRecipe);
  const ref2vaImageField = ref2vaMode ? referenceImagesField(currentRecipe) : undefined;
  const ref2vaMinItems = ref2vaMode ? Math.max(2, ref2vaImageField?.minItems ?? 0) : undefined;
  const referenceValidation = ref2vaMode
    ? validateRef2vaReferences(ref2vaImageField, currentReferences)
    : undefined;
  const imageAssets = assets.filter(isImageAsset);
  const videoAssets = assets.filter(isVideoAsset);

  const applyShot = useCallback((next: ShotView) => {
    setShots((current) => {
      const replaced = current.some((shot) => shot.id === next.id)
        ? current.map((shot) => (shot.id === next.id ? next : shot))
        : [...current, next];
      return replaced.sort((left, right) => left.ordinal - right.ordinal);
    });
    setSelectedShotId(next.id);
  }, []);

  const reload = useCallback(async () => {
    const generation = ++reloadGeneration.current;
    setLoading(true);
    setError(undefined);
    try {
      const [nextShots, nextAssets, promptPage, nextAnchors, nextStructure, nextRunbook, nextPresets] = await Promise.all([
        listShots(projectId),
        listRecentAssets(projectId, 80),
        listPromptLibrary(projectId, { kind: "prompt", limit: 100 }),
        listReferenceAnchors(projectId).catch(() => []),
        listProductionStructure(projectId).catch(() => EMPTY_PRODUCTION_STRUCTURE(projectId)),
        getProductionBatchRunbook({ projectId }).catch(() => emptyRunbook(projectId)),
        listBatchWorkflowPresets().catch(() => []),
      ]);
      if (generation !== reloadGeneration.current) return;
      setShots(nextShots);
      setAssets(nextAssets);
      setReferenceAnchors(nextAnchors);
      setProductionStructure(nextStructure);
      setProductionBatchRunbook(nextRunbook);
      setBatchWorkflowPresets(nextPresets);
      setPromptEntries(promptPage.items);
      setSelectedShotId((current) => {
        const nextSelected = current && nextShots.some((shot) => shot.id === current)
          ? current
          : initialSelectedShotId && nextShots.some((shot) => shot.id === initialSelectedShotId)
            ? initialSelectedShotId
            : nextShots[0]?.id;
        if (nextSelected !== current) onShotSelected?.(nextSelected);
        return nextSelected;
      });
    } catch (loadError: unknown) {
      if (generation !== reloadGeneration.current) return;
      setError(toUserMessage(loadError));
    } finally {
      if (generation === reloadGeneration.current) setLoading(false);
    }
  }, [projectId]);

  useEffect(() => { void reload(); }, [reload]);

  useEffect(() => {
    if (shotList.page === shotListControls.page) return;
    setShotListControls((current) => ({ ...current, page: shotList.page }));
  }, [shotList.page, shotListControls.page]);

  useEffect(() => {
    if (!selectedShot) return;
    setName(selectedShot.name);
    setPromptText(selectedShot.promptText);
    setSelectedPromptId(selectedShot.promptEntryId ?? "");
    setPromptProvenance(
      selectedShot.promptEntryId && selectedShot.promptVersionId
        ? { entryId: selectedShot.promptEntryId, versionId: selectedShot.promptVersionId }
        : undefined,
    );
    const nextDrafts: Partial<Record<ShotStage, StageDraft>> = {};
    for (const nextStage of ["image", "video"] as const) {
      const config = selectedShot.stageConfigs.find((item) => item.stage === nextStage);
      const recipe = config
        ? productCatalog.find((item) =>
          item.workflowVersionId === config.workflowVersionId &&
          item.recipeId === config.recipeId &&
          isProductionRuntimeForStage(nextStage, item.workflowId),
        )
        : preferredStageRecipe(productCatalog, nextStage);
      if (config && recipe) {
        nextDrafts[nextStage] = {
          workflowVersionId: config.workflowVersionId,
          recipeId: config.recipeId,
          values: config.scalarValues as ShotInputValues,
        };
      } else if (recipe) {
        nextDrafts[nextStage] = {
          workflowVersionId: recipe.workflowVersionId,
          recipeId: recipe.recipeId,
          values: defaultScalarValues(recipe),
        };
      }
    }
    setStageDrafts(nextDrafts);
    setDirtyStages(new Set());
    const imageReferences = orderedShotReferences(selectedShot, "image");
    const videoReferences = orderedShotReferences(selectedShot, "video");
    setReferences({
      image: imageReferences,
      video: videoReferences,
    });
  }, [productCatalog, selectedShot]);

  useEffect(() => {
    const linkedIds = selectedShot?.generationLinks.flatMap((link) => link.task?.outputAssetIds ?? []) ?? [];
    const referenceIds = selectedShot?.referenceAssets.map((reference) => reference.assetId) ?? [];
    const missing = [...new Set([...linkedIds, ...referenceIds, selectedShot?.selectedImageAssetId, selectedShot?.selectedVideoAssetId].filter(Boolean) as string[])]
      .filter((id) => !assets.some((asset) => asset.id === id));
    if (!missing.length) return;
    let active = true;
    void Promise.all(missing.slice(0, 40).map((id) => getAsset(projectId, id).catch(() => undefined)))
      .then((loaded) => {
        if (!active) return;
        setAssets((current) => [...current, ...loaded.filter((asset): asset is AssetView => Boolean(asset))]);
      });
    return () => { active = false; };
  }, [assets, projectId, selectedShot]);

  function markStageDirty(nextStage: ShotStage) {
    setDirtyStages((current) => new Set(current).add(nextStage));
  }

  function changeStageRecipe(recipeId: string) {
    const recipe = stageRecipes.find((item) => item.recipeId === recipeId);
    if (!recipe) return;
    const wasRef2va = stage === "video" && isRef2vaRecipe(currentRecipe);
    setStageDrafts((current) => ({
      ...current,
      [stage]: {
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        values: defaultScalarValues(recipe),
      },
    }));
    if (stage === "video" && isRef2vaRecipe(recipe) && !wasRef2va) {
      setReferences((current) => ({
        ...current,
        video: ensurePrimaryReference(current.video, selectedShot?.selectedImageAssetId),
      }));
      if (selectedShot?.selectedImageAssetId) {
        setNotice("已将当前关键帧放到 REF2VA 的 @图片1；其余参考图顺序保持不变。 ");
      }
    } else if (stage === "video" && isRef2vaRecipe(currentRecipe)) {
      setNotice("已切回 I2V；当前已选关键帧保持不变，不会被参考图顺序覆盖。 ");
    }
    markStageDirty(stage);
  }

  function changeScalar(field: RecipeField, value: DraftValue | undefined) {
    if (!currentDraft || (field.type !== "integer" && field.type !== "seed")) return;
    if (!value || (value.type !== "integer" && value.type !== "seed_random" && value.type !== "seed_fixed")) return;
    setStageDrafts((current) => ({
      ...current,
      [stage]: { ...currentDraft, values: { ...currentDraft.values, [field.key]: value } },
    }));
    markStageDirty(stage);
  }

  async function save() {
    if (!selectedShot) return;
    setBusy(true); setError(undefined); setNotice(undefined);
    try {
      let next = await updateShot({
        projectId,
        shotId: selectedShot.id,
        name,
        promptText,
        promptEntryId: promptProvenance?.entryId,
        promptVersionId: promptProvenance?.versionId,
      });
      for (const nextStage of ["image", "video"] as const) {
        const draft = stageDrafts[nextStage];
        if (!draft || !dirtyStages.has(nextStage)) continue;
        next = await setShotStageConfig({ projectId, shotId: next.id, stage: nextStage, ...draft });
      }
      applyShot(next);
      setDirtyStages(new Set());
      setNotice("镜头设置已保存；提示词已保存为当前快照。后续 Prompt Library 更新不会自动改动此镜头。");
    } catch (saveError: unknown) {
      setError(toUserMessage(saveError));
    } finally {
      setBusy(false);
    }
  }

  async function addShot() {
    setBusy(true); setError(undefined);
    try {
      const next = await createShot(projectId);
      applyShot(next);
    } catch (createError: unknown) { setError(toUserMessage(createError)); }
    finally { setBusy(false); }
  }

  async function removeShot() {
    if (!selectedShot || !window.confirm(`确定删除“${selectedShot.name}”？只会删除镜头编排元数据。`)) return;
    setBusy(true); setError(undefined);
    try {
      await deleteShot(projectId, selectedShot.id);
      const remaining = shots.filter((shot) => shot.id !== selectedShot.id);
      setShots(remaining);
      setSelectedShotId(remaining[0]?.id);
    } catch (deleteError: unknown) { setError(toUserMessage(deleteError)); }
    finally { setBusy(false); }
  }

  async function moveShot(delta: -1 | 1) {
    if (!selectedShot || isShotListReorderDisabled(shotListControls)) return;
    const index = shots.findIndex((shot) => shot.id === selectedShot.id);
    const target = index + delta;
    if (index < 0 || target < 0 || target >= shots.length) return;
    const ordered = shots.map((shot) => shot.id);
    [ordered[index], ordered[target]] = [ordered[target], ordered[index]];
    setBusy(true);
    try { setShots(await reorderShots(projectId, ordered)); }
    catch (reorderError: unknown) { setError(toUserMessage(reorderError)); }
    finally { setBusy(false); }
  }

  async function replaceReferences() {
    if (!selectedShot) return;
    if (ref2vaMode && referenceValidation) {
      setError(referenceValidation);
      return;
    }
    setBusy(true); setError(undefined);
    try {
      const next = await replaceShotReferences({ projectId, shotId: selectedShot.id, stage, assetIds: currentReferences });
      applyShot(next);
      setNotice("Reference 素材已保存为关系；不会复制素材文件。");
    } catch (referenceError: unknown) { setError(toUserMessage(referenceError)); }
    finally { setBusy(false); }
  }

  async function applyReferenceAnchor(mode: "append" | "replace") {
    if (!selectedShot || !selectedAnchor) return;
    const anchorAssetIds = selectedAnchor.assets.map((asset) => asset.assetId);
    const result = mode === "append"
      ? appendAnchorReferences(currentReferences, anchorAssetIds)
      : replaceWithAnchorReferences(anchorAssetIds);
    if (!result.ok) {
      setError(result.error);
      return;
    }
    const nextReferences = ref2vaMode
      ? ensurePrimaryReference(result.assetIds, selectedShot.selectedImageAssetId)
      : result.assetIds;
    const nextValidation = ref2vaMode
      ? validateRef2vaReferences(ref2vaImageField, nextReferences)
      : undefined;
    if (nextValidation) {
      setError(nextValidation);
      return;
    }
    setBusy(true); setError(undefined); setNotice(undefined);
    try {
      const next = await replaceShotReferences({
        projectId,
        shotId: selectedShot.id,
        stage,
        assetIds: nextReferences,
      });
      applyShot(next);
      setNotice(`${mode === "append" ? "已追加" : "已替换为"}参考锚点“${selectedAnchor.name}”；Shot 仅保存 asset IDs。`);
    } catch (referenceError: unknown) { setError(toUserMessage(referenceError)); }
    finally { setBusy(false); }
  }

  async function selectResult(assetId: string, fromLinkedTask: boolean) {
    if (!selectedShot) return;
    setBusy(true); setError(undefined);
    try { applyShot(await selectShotResult({ projectId, shotId: selectedShot.id, stage, assetId, fromLinkedTask })); }
    catch (selectError: unknown) { setError(toUserMessage(selectError)); }
    finally { setBusy(false); }
  }

  async function generate() {
    if (!selectedShot || !currentDraft) return;
    if (stage === "video") {
      if (!ref2vaMode && !selectedShot.selectedImageAssetId) {
        setError("请先选择关键帧图片");
        return;
      }
      if (referenceValidation) {
        setError(referenceValidation);
        return;
      }
    }
    setBusy(true); setError(undefined); setNotice(undefined);
    try {
      if (ref2vaMode && !sameReferenceOrder(currentReferences, orderedShotReferences(selectedShot, "video"))) {
        const next = await replaceShotReferences({ projectId, shotId: selectedShot.id, stage: "video", assetIds: currentReferences });
        applyShot(next);
      }
      const task = await generateShot({ projectId, shotId: selectedShot.id, stage, values: currentDraft.values });
      setNotice(`任务 ${task.id} 已创建；Shot 状态由任务和候选素材派生。不会自动跳过候选选择。`);
      applyShot(await getShot(projectId, selectedShot.id));
    } catch (generateError: unknown) { setError(toUserMessage(generateError)); }
    finally { setBusy(false); }
  }

  async function selectBatchResult(shotId: string, resultStage: ShotStage, assetId: string, fromLinkedTask: boolean) {
    setBusy(true); setError(undefined);
    try {
      applyShot(await selectShotResult({ projectId, shotId, stage: resultStage, assetId, fromLinkedTask }));
      setNotice(`${resultStage === "image" ? "关键帧" : "最终视频"}已确认；不会自动提交下一阶段。`);
    } catch (selectError: unknown) {
      setError(toUserMessage(selectError));
    } finally {
      setBusy(false);
    }
  }

  async function retryShot(shotId: string, retryStage: ShotStage) {
    const shot = shots.find((item) => item.id === shotId);
    const failedLink = shot?.generationLinks.find((link) => link.stage === retryStage && link.task?.status === "FAILED");
    if (!shot || !failedLink) return;
    setBusy(true); setError(undefined);
    try {
      if (failedLink.productionBatchItemId) {
        const detail = await requeueProductionQueueItemByItem(projectId, failedLink.productionBatchItemId);
        await startProductionQueue(projectId, detail.id);
        setNotice("已创建新的普通队列项并开始处理；原失败任务和关联记录已保留。新队列仍按阶段严格串行。 ");
      } else {
        const task = await generateShot({ projectId, shotId, stage: retryStage, retryTaskId: failedLink.task?.id });
        setNotice(`已创建新的普通任务 ${task.id}；原失败任务仍保留。`);
      }
      await reload();
    } catch (retryError: unknown) {
      setError(toUserMessage(retryError));
    } finally {
      setBusy(false);
    }
  }

  function loadPrompt() {
    const entry = promptEntries.find((item) => item.id === selectedPromptId);
    const version = entry?.versions[entry.versions.length - 1];
    if (!entry || !version) return;
    if (entry.kind === "prompt" && isPromptTemplateText(version.text)) {
      setNotice("这是模板 Prompt，请在下方预览并确认后应用；不会把 {{variable}} 原样保存到镜头。");
      return;
    }
    setPromptText(version.text);
    setPromptProvenance({ entryId: entry.id, versionId: version.id });
    setNotice(`已载入 Prompt Library「${entry.name}」的 v${version.version}；之后编辑会清除来源标记。`);
  }

  async function exportManifest() {
    setBusy(true);
    setError(undefined);
    try {
      const exported = await exportProjectManifest(projectId);
      if (exported) setNotice(`项目清单已导出：${exported.fileName}`);
    } catch (exportError: unknown) {
      setError(toUserMessage(exportError));
    } finally {
      setBusy(false);
    }
  }

  const stageLinks = selectedShot?.generationLinks.filter((link) => link.stage === stage) ?? [];
  const stageCandidateIds = new Set(stageLinks.flatMap((link) => link.task?.outputAssetIds ?? []));
  const stageCandidates = assets.filter((asset) => stageCandidateIds.has(asset.id) && (stage === "image" ? isImageAsset(asset) : isVideoAsset(asset)));
  const manualAssets = stage === "image" ? imageAssets : videoAssets;
  const hasFrozenQueueLink = Boolean(selectedShot?.generationLinks.some((link) => link.productionBatchItemId));
  const recentFailure = selectedShot ? recentShotFailure(selectedShot, stage) : undefined;

  async function configureBulkStage(nextStage: ShotStage, shotIds: string[]) {
    const recipe = preferredStageRecipe(productCatalog, nextStage);
    if (!recipe) throw new Error(`当前没有可用的${nextStage === "image" ? "Krea2 图片" : "MiniMax H3 视频"} Recipe。`);
    await bulkSetShotStageConfig({
      projectId,
      stage: nextStage,
      shotIds,
      workflowVersionId: recipe.workflowVersionId,
      recipeId: recipe.recipeId,
      values: defaultScalarValues(recipe),
    });
  }

  async function assignBulkPrompt(nextStage: ShotStage, shotIds: string[], text: string) {
    await bulkAssignShotPrompt({
      projectId,
      stage: nextStage,
      shotIds,
      source: { type: "text", text },
    });
  }

  if (loading) return <section className="workspace-panel shot-workspace"><p className="project-loading">正在加载镜头制作...</p></section>;

  return (
    <section className="workspace-panel shot-workspace" aria-busy={busy}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">Shot production</span>
          <h2>镜头制作</h2>
          <p className="section-description">关键帧候选 → 选定关键帧 → Reference Image → 视频候选 → 选定最终视频。</p>
        </div>
        <div className="shot-toolbar">
          <button type="button" onClick={() => void addShot()} disabled={busy}>新建镜头</button>
          <button type="button" className="quiet-button" onClick={() => setBulkImportOpen((open) => !open)} disabled={busy}>{bulkImportOpen ? "收起批量导入" : "批量导入镜头"}</button>
          <button type="button" className="quiet-button" onClick={() => void exportManifest()} disabled={busy}>导出项目清单</button>
          <button type="button" className="quiet-button" onClick={() => void reload()} disabled={busy}>刷新</button>
        </div>
      </div>
      {bulkImportOpen && <ShotBulkImportPanel
        projectId={projectId}
        onImported={async () => { await reload(); setBulkImportOpen(false); }}
        onCancel={() => setBulkImportOpen(false)}
      />}
      <ProductionStructurePanel
        projectId={projectId}
        tree={productionStructure}
        shots={shots}
        selectedShotId={selectedShotId}
        onSelectShot={setSelectedShotId}
        onChanged={setProductionStructure}
        onError={(message) => setError(message || undefined)}
      />
      <EpisodeProductionPanel
        projectId={projectId}
        tree={productionStructure}
        shots={shots}
        promptEntries={promptEntries}
        referenceAnchors={referenceAnchors}
        onRefresh={reload}
        onNotice={(message) => setNotice(message)}
        onError={(message) => setError(message || undefined)}
        onOpenProductionQueue={onOpenProductionQueue}
        onNavigateToScene={(sceneId) => {
          const scene = productionStructure.series.flatMap((series) => series.episodes.flatMap((episode) => episode.scenes)).find((item) => item.id === sceneId);
          const firstShotId = scene?.shotIds[0];
          if (firstShotId) {
            setSelectedShotId(firstShotId);
            onShotSelected?.(firstShotId);
          }
        }}
      />
      <SeriesProductionPanel
        projectId={projectId}
        tree={productionStructure}
        shots={shots}
        promptEntries={promptEntries}
        referenceAnchors={referenceAnchors}
        initialPresets={batchWorkflowPresets}
        onRefresh={reload}
        onNotice={(message) => setNotice(message)}
        onError={(message) => setError(message || undefined)}
        onOpenProductionQueue={onOpenProductionQueue}
        onNavigateToEpisode={(episodeId) => {
          const firstShotId = firstShotForEpisode(productionStructure, episodeId);
          if (firstShotId) { setSelectedShotId(firstShotId); onShotSelected?.(firstShotId); }
        }}
        onPlan={getSeriesProductionPlan}
        onPrepare={prepareSeriesProduction}
        onApplyPreset={async (request: SeriesPresetApplyRequest) => {
          await bulkSetShotStageConfig({
            projectId: request.projectId,
            stage: request.stage,
            shotIds: request.shotIds,
            workflowVersionId: request.workflowVersionId,
            recipeId: request.recipeId,
            values: request.values,
          });
          await reload();
        }}
        onPreviewPrompt={async (request: SeriesPromptBulkRequest) => {
          const preview = await previewPromptTemplateBulk({
            projectId: request.projectId,
            promptEntryId: request.promptEntryId,
            promptVersionId: request.promptVersionId,
            shotIds: request.shotIds,
            contextAnchorIds: request.contextAnchorIds,
            customValues: request.customValues,
            previewLimit: 20,
          });
          return {
            total: preview.total,
            valid: preview.valid,
            invalid: preview.invalid,
            samples: preview.previewEntries.map((entry) => ({ shotId: entry.shotId, text: entry.renderedText, valid: true })),
          };
        }}
        onApplyPrompt={async (request: SeriesPromptBulkRequest) => {
          await applyPromptTemplate({
            projectId: request.projectId,
            promptEntryId: request.promptEntryId,
            promptVersionId: request.promptVersionId,
            stage: request.stage,
            shotIds: request.shotIds,
            contextAnchorIds: request.contextAnchorIds,
            customValues: request.customValues,
          });
          await reload();
        }}
      />
      <ProductionBatchRunbookPanel
        projectId={projectId}
        runbook={productionBatchRunbook}
        onRefresh={reload}
        onStartBatch={async (batchId) => { await startProductionQueue(projectId, batchId); await reload(); }}
        onOpenProductionQueue={onOpenProductionQueue ? () => onOpenProductionQueue() : undefined}
        onNavigateToEpisode={(episodeId) => {
          const firstShotId = firstShotForEpisode(productionStructure, episodeId);
          if (firstShotId) { setSelectedShotId(firstShotId); onShotSelected?.(firstShotId); }
        }}
        onNavigateToScene={(sceneId) => {
          const firstShotId = firstShotForScene(productionStructure, sceneId);
          if (firstShotId) { setSelectedShotId(firstShotId); onShotSelected?.(firstShotId); }
        }}
      />
      <ProjectProductionPipeline
        projectId={projectId}
        shots={shots}
        onRefresh={reload}
        onNotice={(message) => setNotice(message)}
        onError={(message) => setError(message)}
        onConfigureStage={configureBulkStage}
        onBulkPrompt={assignBulkPrompt}
        busy={busy}
        onOpenReview={(reviewStage, shotIds) => {
          if (busy || !shotIds.length) return;
          setStage(reviewStage);
          setSelectedShotId(shotIds[0]);
        }}
      />
      <SceneProductionPanel
        projectId={projectId}
        sceneOptions={sceneFilterOptions.filter((option) => option.value !== "ALL" && option.value !== "UNASSIGNED")}
        currentSceneId={selectedSceneContext?.scene.id}
        currentShot={selectedShot}
        promptEntries={promptEntries}
        referenceAnchors={referenceAnchors}
        onRefresh={reload}
        onNotice={(message) => setNotice(message)}
        onNavigateToReview={(reviewStage) => setStage(reviewStage)}
      />
      <div className="shot-workspace-grid">
        <aside className="shot-list-pane" aria-label="镜头列表">
          <div className="shot-pane-heading"><strong>镜头列表</strong><span>{shotList.filteredCount} / {shots.length}</span></div>
          <ShotListToolbar
            controls={shotListControls}
            filteredCount={shotList.filteredCount}
            totalCount={shots.length}
            pageStart={shotList.pageStart}
            pageEnd={shotList.pageEnd}
            pageCount={shotList.pageCount}
            onQueryChange={(query) => setShotListControls((current) => updateShotListControls(current, { query }))}
            onStatusChange={(status) => setShotListControls((current) => updateShotListControls(current, { status }))}
            sceneOptions={sceneFilterOptions}
            onSceneChange={(sceneId) => setShotListControls((current) => updateShotListControls(current, { sceneId }))}
            onPageSizeChange={(pageSize) => setShotListControls((current) => updateShotListControls(current, { pageSize }))}
            onPageChange={(page) => setShotListControls((current) => ({ ...current, page: Math.max(1, Math.min(page, shotList.pageCount)) }))}
          />
          {selectedShot && shotList.isFiltered && !shotList.filteredShots.some((shot) => shot.id === selectedShot.id) && <p className="shot-selection-filter-note">当前详情仍显示已选镜头；清除筛选后可在列表中定位。</p>}
          {shots.length === 0 && <p className="empty-state">还没有镜头，先新建一个。</p>}
          {shots.length > 0 && !shotList.filteredCount && <p className="empty-state">没有匹配的镜头，请清除搜索或状态筛选。</p>}
          {shotList.pageShots.map((item) => {
            const derived = deriveShotStatus(item);
            return (
              <button key={item.id} type="button" className={`shot-list-item${item.id === selectedShotId ? " shot-list-item-active" : ""}`} onClick={() => { setSelectedShotId(item.id); onShotSelected?.(item.id); }}>
                <span className="shot-list-number">{String(item.ordinal + 1).padStart(2, "0")}</span>
                <span className="shot-list-copy"><strong>{item.name}</strong><small>{shotStatusLabels[derived]}</small></span>
              </button>
            );
          })}
        </aside>

        {!selectedShot ? (
          <div className="shot-empty-editor"><strong>选择一个镜头开始制作</strong><p>镜头会保存 Prompt 快照、阶段配置、Reference 关系和任务关联。</p></div>
        ) : (
          <div className="shot-editor-pane">
            <div className="shot-editor-header">
              <div><span className="section-label">镜头 {String(selectedShot.ordinal + 1).padStart(2, "0")}</span><h3>{selectedShot.name}</h3></div>
              <div className="shot-editor-actions">
                <button type="button" className="quiet-button" onClick={() => void moveShot(-1)} disabled={busy || isShotListReorderDisabled(shotListControls) || selectedShot.ordinal === 0}>上移</button>
                <button type="button" className="quiet-button" onClick={() => void moveShot(1)} disabled={busy || isShotListReorderDisabled(shotListControls) || selectedShot.ordinal === shots.length - 1}>下移</button>
                <button type="button" className="quiet-button danger-button" onClick={() => void removeShot()} disabled={busy}>删除镜头</button>
                <button type="button" onClick={() => void save()} disabled={busy}>保存镜头</button>
              </div>
            </div>
            {shotList.isFiltered && <p className="shot-reorder-filter-note">清除筛选后可调整全局顺序。</p>}
            <div className="shot-editor-body">
              <div className="shot-settings-column">
                <label><span>镜头名称</span><input value={name} onChange={(event) => setName(event.target.value)} /></label>
                <label><span>Prompt 快照</span><textarea value={promptText} onChange={(event) => { setPromptText(event.target.value); setPromptProvenance(undefined); setSelectedPromptId(""); }} rows={6} placeholder="描述镜头画面、动作和构图" /></label>
                <div className="shot-prompt-loader">
                  <label><span>从 Prompt Library 载入</span><select value={selectedPromptId} onChange={(event) => setSelectedPromptId(event.target.value)}><option value="">选择提示词</option>{promptEntries.map((entry) => <option key={entry.id} value={entry.id}>{entry.name} · {entry.versions.length} 版</option>)}</select></label>
                  <button type="button" className="quiet-button" onClick={loadPrompt} disabled={!selectedPromptId}>载入快照</button>
                </div>
                {selectedPromptEntry?.kind === "prompt" && selectedPromptVersion && isPromptTemplateText(selectedPromptVersion.text) && <PromptTemplatePanel
                  projectId={projectId}
                  projectName={projectName}
                  projectDescription={projectDescription}
                  stage={stage}
                  entry={selectedPromptEntry}
                  version={selectedPromptVersion}
                  shot={selectedShot}
                  structureContext={selectedSceneContext}
                  referenceAnchors={referenceAnchors}
                  onApplied={() => reload()}
                  disabled={busy}
                />}
                {promptProvenance && <p className="shot-provenance">来源：Prompt Library · version {promptProvenance.versionId.slice(-8)}</p>}
                <div className="shot-stage-tabs" role="tablist" aria-label="制作阶段">
                  <button type="button" className={stage === "image" ? "active" : ""} onClick={() => setStage("image")}>关键帧图片</button>
                  <button type="button" className={stage === "video" ? "active" : ""} onClick={() => setStage("video")}>参考图生视频</button>
                </div>
                {hasFrozenQueueLink && <p className="shot-frozen-config-warning"><strong>已有生产队列快照</strong>：本次配置编辑只影响后续新批次，已入队的任务继续使用冻结配置。</p>}
                <label><span>{stage === "image" ? "图片阶段 Recipe" : "视频阶段 Recipe"}</span><select value={currentDraft?.recipeId ?? ""} onChange={(event) => changeStageRecipe(event.target.value)}><option value="">选择兼容输出</option>{stageRecipes.map((recipe) => <option key={`${recipe.workflowVersionId}:${recipe.recipeId}`} value={recipe.recipeId}>{recipe.name} · {recipe.mode}</option>)}</select></label>
                {currentRecipe && <p className="shot-recipe-hint">按精确产品运行时和输出能力筛选；不会根据显示名称判断运行时。{ref2vaMode ? `REF2VA 参考图 ${ref2vaMinItems ?? 2}～${ref2vaImageField?.maxItems ?? 9} 张，按 @图片顺序提交。` : "I2V 使用当前已选关键帧，不会维护多图列表。"}</p>}
                {currentRecipe?.fields.filter(isScalarField).map((field) => <ScalarControl key={field.key} field={field} value={currentDraft?.values[field.key]} onChange={(value) => changeScalar(field, value)} />)}
                <button type="button" className="shot-primary-action" onClick={() => void generate()} disabled={busy || !currentDraft || (stage === "video" && ((!ref2vaMode && !selectedShot.selectedImageAssetId) || Boolean(referenceValidation)))}>{stage === "image" ? "生成关键帧" : "生成视频"}</button>
                <button type="button" className="quiet-button" onClick={() => currentRecipe && onOpenInStudio(selectedShot, stage, currentRecipe)}>在创作中打开</button>
                {stage === "video" && <p className="shot-inline-note">{referenceValidation ?? "生成视频时会保留当前关键帧选择；不会在图片成功后自动提交视频任务。"}</p>}
              </div>
              <div className="shot-results-column">
                <section className="shot-panel-block">
                  <div className="shot-block-heading">
                    <div><span className="section-label">Reference</span><h4>{ref2vaMode ? "有序参考图" : "参考素材关系"}</h4></div>
                    {(stage === "image" || ref2vaMode) && <button type="button" className="quiet-button" onClick={() => void replaceReferences()} disabled={busy}>保存关系</button>}
                  </div>
                  {(stage === "image" || ref2vaMode) && <div className="shot-anchor-picker">
                    <label>
                      <span>参考锚点</span>
                      <select value={selectedAnchorId} onChange={(event) => setSelectedAnchorId(event.target.value)} disabled={busy}>
                        <option value="">选择可复用的参考锚点</option>
                        {referenceAnchors.map((anchor) => <option key={anchor.id} value={anchor.id} disabled={!anchor.usable || anchor.assets.length === 0}>
                          [{anchor.kind}] {anchor.name}{anchor.usable && anchor.assets.length > 0 ? ` · ${anchor.assets.length} 张` : " · 暂无参考图"}
                        </option>)}
                      </select>
                    </label>
                    <div className="shot-anchor-actions">
                      <button type="button" className="quiet-button" onClick={() => void applyReferenceAnchor("append")} disabled={busy || !selectedAnchor?.usable || !selectedAnchor.assets.length}>追加锚点</button>
                      <button type="button" className="quiet-button" onClick={() => void applyReferenceAnchor("replace")} disabled={busy || !selectedAnchor?.usable || !selectedAnchor.assets.length}>替换为锚点</button>
                    </div>
                    <p className="shot-inline-note">套用后只写入当前参考图 asset IDs；锚点之后的修改不会影响此 Shot。</p>
                  </div>}
                  {stage === "image" ? (
                    <>
                      <p className="shot-inline-note">仅保存当前项目素材 ID，不复制文件。图片阶段可选多个。</p>
                      <div className="shot-asset-checklist">{imageAssets.slice(0, 24).map((asset) => <label key={asset.id}><input type="checkbox" checked={currentReferences.includes(asset.id)} onChange={() => setReferences((current) => ({ ...current, image: toggleOrderedReference(current.image, asset.id) }))} /><AssetThumb projectId={projectId} asset={asset} /><span>{asset.name}</span></label>)}{imageAssets.length === 0 && <span className="empty-state">当前项目暂无图片素材。</span>}</div>
                    </>
                  ) : ref2vaMode ? (
                    <>
                      <p className="shot-inline-note">按顺序提交给 H3；切换到 REF2VA 时已选关键帧会明确显示为 @图片1。达到 Recipe 上限后不能继续添加。</p>
                      <div className="shot-asset-checklist">
                        {currentReferences.map((assetId, index) => {
                          const asset = imageAssets.find((item) => item.id === assetId);
                          return <label key={assetId}><span>@图片{index + 1}</span>{asset ? <AssetThumb projectId={projectId} asset={asset} /> : <span className="shot-thumb">图片</span>}<span>{asset?.name ?? assetId}</span><button type="button" className="quiet-button" onClick={() => setReferences((current) => ({ ...current, video: moveOrderedReference(current.video, index, -1) }))} disabled={busy || index === 0}>上移</button><button type="button" className="quiet-button" onClick={() => setReferences((current) => ({ ...current, video: moveOrderedReference(current.video, index, 1) }))} disabled={busy || index === currentReferences.length - 1}>下移</button><button type="button" className="quiet-button" onClick={() => setReferences((current) => ({ ...current, video: removeOrderedReference(current.video, assetId) }))} disabled={busy}>移除</button></label>;
                        })}
                        {!currentReferences.length && <span className="empty-state">尚未添加参考图。</span>}
                      </div>
                      <div className="shot-asset-checklist">{imageAssets.filter((asset) => !currentReferences.includes(asset.id)).slice(0, 24).map((asset) => <label key={asset.id}><AssetThumb projectId={projectId} asset={asset} /><span>{asset.name}</span><button type="button" className="quiet-button" onClick={() => setReferences((current) => ({ ...current, video: addOrderedReference(current.video, asset.id, ref2vaImageField?.maxItems) }))} disabled={busy || currentReferences.length >= (ref2vaImageField?.maxItems ?? 0)}>添加为 @图片{currentReferences.length + 1}</button></label>)}</div>
                    </>
                  ) : (
                    <>
                      <p className="shot-inline-note">I2V 使用当前已选关键帧；切回 I2V 不会静默覆盖该选择。</p>
                      {selectedShot.selectedImageAssetId ? <div className="shot-asset-checklist"><label><span>@图片1 · 当前关键帧</span>{imageAssets.find((asset) => asset.id === selectedShot.selectedImageAssetId) ? <AssetThumb projectId={projectId} asset={imageAssets.find((asset) => asset.id === selectedShot.selectedImageAssetId)!} /> : <span className="shot-thumb">图片</span>}<span>{selectedShot.selectedImageAssetId}</span></label></div> : <p className="empty-state">请先在关键帧图片阶段选择图片。</p>}
                    </>
                  )}
                </section>
                <section className="shot-panel-block"><div className="shot-block-heading"><div><span className="section-label">Candidates</span><h4>{stage === "image" ? "图片候选" : "视频候选"}</h4></div></div><div className="shot-candidate-grid">{stageCandidates.map((asset) => <CandidateCard key={asset.id} projectId={projectId} asset={asset} selected={stage === "image" ? selectedShot.selectedImageAssetId === asset.id : selectedShot.selectedVideoAssetId === asset.id} onSelect={() => void selectResult(asset.id, true)} disabled={busy} label={stage === "image" ? "设为关键帧" : "设为最终视频"} />)}{stageCandidates.length === 0 && <p className="empty-state">暂无该阶段任务候选；生成后结果会出现在这里。</p>}</div></section>
                <section className="shot-panel-block"><div className="shot-block-heading"><div><span className="section-label">History</span><h4>生成历史</h4></div></div>{recentFailure && <div className="shot-recent-failure"><strong>最近失败记录（辅助信息）</strong><span>{recentFailure.error?.message ?? "任务失败"}</span>{onOpenTask && <button type="button" className="quiet-button" onClick={() => onOpenTask(recentFailure.id)}>查看任务详情</button>}</div>}<div className="shot-history-list">{stageLinks.slice(0, 8).map((link) => <div key={link.id} className="shot-history-item"><span>{formatDateTime(link.createdAt)}</span><strong>{link.task?.status ?? "关联中"}</strong><small>{link.task?.id ?? link.id}</small></div>)}{stageLinks.length === 0 && <p className="empty-state">尚无生成任务。</p>}</div></section>
                <section className="shot-panel-block"><div className="shot-block-heading"><div><span className="section-label">Project assets</span><h4>当前项目素材</h4></div></div><div className="shot-manual-assets">{manualAssets.slice(0, 12).map((asset) => <CandidateCard key={asset.id} projectId={projectId} asset={asset} selected={stage === "image" ? selectedShot.selectedImageAssetId === asset.id : selectedShot.selectedVideoAssetId === asset.id} onSelect={() => void selectResult(asset.id, false)} disabled={busy} label={stage === "image" ? "设为关键帧" : "设为最终视频"} />)}</div></section>
              </div>
            </div>
          </div>
        )}
      </div>
      <ShotBatchReviewBoard
        projectId={projectId}
        shots={shots}
        assets={assets}
        stage={stage}
        busy={busy}
        onAssetsLoaded={(loaded) => setAssets((current) => [...current, ...loaded.filter((asset) => !current.some((item) => item.id === asset.id))])}
        onSelect={(shotId, reviewStage, assetId, fromLinkedTask) => void selectBatchResult(shotId, reviewStage, assetId, fromLinkedTask)}
        onRetry={(shotId, reviewStage) => void retryShot(shotId, reviewStage)}
        onOpenTask={onOpenTask}
      />
      {notice && <p className="studio-notice">{notice}</p>}
      {error && <p className="error-message">{error}</p>}
    </section>
  );
}

export function isRef2vaRecipe(recipe?: RecipeViewModel): boolean {
  return recipe ? h3FamilyForWorkflowId(recipe.workflowId) === "REF2VA" : false;
}

export function referenceImagesField(recipe?: RecipeViewModel): Extract<RecipeField, { type: "images" }> | undefined {
  return recipe?.fields.find((field): field is Extract<RecipeField, { type: "images" }> => field.type === "images");
}

export function validateRef2vaReferences(
  field: Extract<RecipeField, { type: "images" }> | undefined,
  assetIds: string[],
): string | undefined {
  if (!field) return "REF2VA Recipe 缺少多图参考输入";
  const minItems = Math.max(2, field.minItems);
  if (new Set(assetIds).size !== assetIds.length) return "REF2VA 参考图不能重复";
  if (assetIds.length < minItems) return `REF2VA 至少需要 ${minItems} 张参考图`;
  if (assetIds.length > field.maxItems) return `REF2VA 最多允许 ${field.maxItems} 张参考图`;
  return undefined;
}

export function ensurePrimaryReference(assetIds: string[], selectedAssetId?: string): string[] {
  if (!selectedAssetId || assetIds[0] === selectedAssetId) return assetIds;
  return [selectedAssetId, ...assetIds.filter((assetId) => assetId !== selectedAssetId)];
}

export function addOrderedReference(assetIds: string[], assetId: string, maxItems?: number): string[] {
  if (assetIds.includes(assetId) || (maxItems !== undefined && assetIds.length >= maxItems)) return assetIds;
  return [...assetIds, assetId];
}

export function toggleOrderedReference(assetIds: string[], assetId: string, maxItems?: number): string[] {
  return assetIds.includes(assetId)
    ? removeOrderedReference(assetIds, assetId)
    : addOrderedReference(assetIds, assetId, maxItems);
}

export function removeOrderedReference(assetIds: string[], assetId: string): string[] {
  return assetIds.filter((current) => current !== assetId);
}

export function moveOrderedReference(assetIds: string[], index: number, delta: -1 | 1): string[] {
  const target = index + delta;
  if (index < 0 || index >= assetIds.length || target < 0 || target >= assetIds.length) return assetIds;
  const next = [...assetIds];
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}

function orderedShotReferences(shot: ShotView, stage: ShotStage): string[] {
  return shot.referenceAssets
    .filter((reference) => reference.stage === stage)
    .sort((left, right) => left.ordinal - right.ordinal)
    .map((reference) => reference.assetId);
}

function sameReferenceOrder(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((assetId, index) => assetId === right[index]);
}

function isImageAsset(asset: AssetView): boolean {
  return asset.assetType === "image" || asset.category === "source_image" || asset.category === "generated_image";
}

function isVideoAsset(asset: AssetView): boolean {
  return asset.assetType === "video" || asset.category === "source_video" || asset.category === "generated_video";
}

function isScalarField(field: RecipeField): field is Extract<RecipeField, { type: "integer" | "seed" }> {
  return field.type === "integer" || field.type === "seed";
}

function defaultScalarValues(recipe: RecipeViewModel): ShotInputValues {
  return Object.fromEntries(recipe.fields.filter(isScalarField).map((field) => {
    if (field.type === "integer") return [field.key, field.default === undefined ? { type: "integer", value: 0 } : { type: "integer", value: field.default }];
    return [field.key, field.defaultMode === "fixed" ? { type: "seed_fixed", value: field.defaultValue ?? "0" } : { type: "seed_random" }];
  }));
}

function preferredStageRecipe(catalog: RecipeViewModel[], stage: ShotStage): RecipeViewModel | undefined {
  const compatible = catalog.filter((item) =>
    isProductionRuntimeForStage(stage, item.workflowId) && (item.outputTypes?.includes(stage) ?? false),
  );
  if (stage === "video") {
    return compatible.find((item) => item.workflowId === MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID)
      ?? compatible.find((item) => h3QualityProfileForWorkflowId(item.workflowId) === "QUALITY")
      ?? compatible[0];
  }
  return compatible[0];
}

function ScalarControl({ field, value, onChange }: { field: RecipeField & { type: "integer" | "seed" }; value?: DraftValue; onChange: (value: DraftValue) => void }) {
  if (field.type === "integer") {
    return <label className="shot-scalar-control"><span>{field.label}</span><input type="number" value={value?.type === "integer" ? value.value : ""} min={field.min} max={field.max} onChange={(event) => onChange({ type: "integer", value: Number(event.target.value) })} /></label>;
  }
  const fixed = value?.type === "seed_fixed";
  return <div className="shot-scalar-control"><span>{field.label}</span><div className="shot-seed-control"><select value={fixed ? "fixed" : "random"} onChange={(event) => onChange(event.target.value === "fixed" ? { type: "seed_fixed", value: field.defaultValue ?? "0" } : { type: "seed_random" })}><option value="random">随机</option><option value="fixed">固定</option></select>{fixed && <input value={value.value} inputMode="numeric" onChange={(event) => onChange({ type: "seed_fixed", value: event.target.value })} />}</div></div>;
}

function AssetThumb({ projectId, asset }: { projectId: string; asset: AssetView }) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    void (asset.thumbnailAvailable ? readAssetThumbnail(projectId, asset.id).catch(() => readAssetImage(projectId, asset.id)) : readAssetImage(projectId, asset.id))
      .then((bytes) => { if (!active) return; objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" })); setUrl(objectUrl); })
      .catch(() => undefined);
    return () => { active = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [asset.id, asset.thumbnailAvailable, projectId]);
  return <span className="shot-thumb">{url ? <img src={url} alt="" /> : <span>图片</span>}</span>;
}

function CandidateCard({ projectId, asset, selected, onSelect, disabled = false, label }: { projectId: string; asset: AssetView; selected: boolean; onSelect: () => void; disabled?: boolean; label: string }) {
  const isVideo = isVideoAsset(asset);
  return <article className={`shot-candidate-card${selected ? " shot-candidate-card-selected" : ""}`}><AssetThumb projectId={projectId} asset={asset} /><div><strong>{asset.name}</strong><small>{asset.id}</small></div><button type="button" disabled={disabled || selected} onClick={onSelect}>{selected ? "已选" : label}</button>{isVideo && <small>视频候选</small>}</article>;
}

function emptyRunbook(projectId: string): ProductionBatchRunbookView {
  return { projectId, rows: [] };
}

function firstShotForEpisode(tree: ProductionStructureTree, episodeId: string): string | undefined {
  return tree.series
    .flatMap((series) => series.episodes)
    .find((episode) => episode.id === episodeId)
    ?.scenes.flatMap((scene) => scene.shotIds)[0];
}

function firstShotForScene(tree: ProductionStructureTree, sceneId: string): string | undefined {
  return tree.series
    .flatMap((series) => series.episodes)
    .flatMap((episode) => episode.scenes)
    .find((scene) => scene.id === sceneId)
    ?.shotIds[0];
}
