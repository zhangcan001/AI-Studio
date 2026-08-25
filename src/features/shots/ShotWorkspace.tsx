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
  pauseProductionQueue,
  requeueProductionQueueItemByItem,
  replaceShotReferences,
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
import type { WorkspaceSelection } from "../../types/workspaceSelection";
import { toUserMessage } from "../../i18n/errorMessages";
import { deriveShotStatus, shotStatusLabels } from "./shotDomain";
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
import { ProductionQueueDrawer } from "../production/ProductionQueueDrawer";
import { ProjectStructureTree, type ProjectStructureCreateTarget } from "./ProjectStructureTree";
import { ShotCreationWorkspace, type ShotCreationWorkspaceTab, type ShotWorkspaceCandidate } from "./ShotCreationWorkspace";
import type { ShotInspectorTab } from "./ShotInspector";
import {
  appendAnchorReferences,
  replaceWithAnchorReferences,
} from "./referenceAnchorApply";
import {
  buildShotListView,
  defaultShotListControls,
  updateShotListControls,
  type ShotListControls,
} from "./shotListQuery";
import { EMPTY_PRODUCTION_STRUCTURE, findProductionSceneParent, orderedEpisodes, orderedSeries, productionSceneOptions, shotSceneIndex } from "./productionStructureState";
import { isPromptTemplateText } from "../prompts/promptTemplateState";
import {
  filterProductionRuntimeCatalog,
  h3FamilyForWorkflowId,
  h3QualityProfileForWorkflowId,
  isProductionRuntimeForStage,
  MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
} from "../runtime/productRuntimeScope";
import "./ShotWorkspace.css";

export type ShotWorkspaceMode = "creation" | "production" | "review";

export interface ShotContextPathItem {
  type: Exclude<WorkspaceSelection["type"], "project">;
  id: string;
  label: string;
}

export type ShotContextSurface = "project" | "series" | "episode" | "scene" | "shot" | "production" | "review";

export function shotContextSurface(mode: ShotWorkspaceMode, selectionType: WorkspaceSelection["type"]): ShotContextSurface {
  if (mode === "production") return "production";
  if (mode === "review") return "review";
  return selectionType;
}

interface Props {
  projectId: string;
  projectName?: string;
  projectDescription?: string | null;
  catalog: RecipeViewModel[];
  initialSelectedShotId?: string;
  mode?: ShotWorkspaceMode;
  onShotSelected?: (shotId?: string) => void;
  onContextPathChange?: (path: ShotContextPathItem[]) => void;
  contextPathTarget?: ShotContextPathItem;
  onOpenTask?: (taskId: string) => void;
  onOpenProductionQueue?: () => void;
}

type StageDraft = {
  workflowVersionId: string;
  recipeId: string;
  values: ShotInputValues;
};

const emptyStageDrafts: Partial<Record<ShotStage, StageDraft>> = {};

export function ShotWorkspace({ projectId, projectName, projectDescription, catalog, initialSelectedShotId, mode = "creation", onShotSelected, onContextPathChange, contextPathTarget, onOpenTask, onOpenProductionQueue }: Props) {
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
  const [workspaceSelection, setWorkspaceSelection] = useState<WorkspaceSelection>(() => initialSelectedShotId
    ? { type: "shot", shotId: initialSelectedShotId }
    : { type: "project", projectId });
  const [shotWorkspaceTab, setShotWorkspaceTab] = useState<ShotCreationWorkspaceTab>("generate");
  const [inspectorTab, setInspectorTab] = useState<ShotInspectorTab>("parameters");
  const [previewAssetId, setPreviewAssetId] = useState<string>();
  const [structureManagementOpen, setStructureManagementOpen] = useState(false);
  const [name, setName] = useState("");
  const [promptText, setPromptText] = useState("");
  const [promptProvenance, setPromptProvenance] = useState<{ entryId: string; versionId: string }>();
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [bulkImportOpen, setBulkImportOpen] = useState(false);
  const [structureMenuOpen, setStructureMenuOpen] = useState(false);
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
  const workspaceSceneId = workspaceSelection.type === "scene"
    ? workspaceSelection.sceneId
    : workspaceSelection.type === "shot"
      ? shotSceneIds[workspaceSelection.shotId]
      : undefined;
  const workspaceSceneContext = workspaceSceneId
    ? findProductionSceneParent(productionStructure, workspaceSceneId)
    : selectedSceneContext;
  const contextPath = useMemo(
    () => buildShotContextPath(productionStructure, workspaceSelection, shots),
    [productionStructure, shots, workspaceSelection],
  );
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
  const referenceValidation = ref2vaMode
    ? validateRef2vaReferences(ref2vaImageField, currentReferences)
    : undefined;
  const imageAssets = assets.filter(isImageAsset);
  const videoAssets = assets.filter(isVideoAsset);

  useEffect(() => {
    setPreviewAssetId(undefined);
  }, [selectedShotId, stage]);

  const applyShot = useCallback((next: ShotView) => {
    setShots((current) => {
      const replaced = current.some((shot) => shot.id === next.id)
        ? current.map((shot) => (shot.id === next.id ? next : shot))
        : [...current, next];
      return replaced.sort((left, right) => left.ordinal - right.ordinal);
    });
    setSelectedShotId(next.id);
    setWorkspaceSelection({ type: "shot", shotId: next.id });
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
    onContextPathChange?.(contextPath);
  }, [contextPath, onContextPathChange]);

  useEffect(() => () => {
    onContextPathChange?.([]);
  }, [onContextPathChange]);

  useEffect(() => {
    if (!contextPathTarget) return;
    selectWorkspaceSelection(selectionForContextPathItem(contextPathTarget));
  }, [contextPathTarget]);

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
    if (!currentDraft || (field.type !== "integer" && field.type !== "number" && field.type !== "seed")) return;
    if (!value || (value.type !== "integer" && value.type !== "number" && value.type !== "seed_random" && value.type !== "seed_fixed")) return;
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
      const nextSelectedShotId = remaining[0]?.id;
      setSelectedShotId(nextSelectedShotId);
      setWorkspaceSelection(nextSelectedShotId
        ? { type: "shot", shotId: nextSelectedShotId }
        : { type: "project", projectId });
      onShotSelected?.(nextSelectedShotId);
    } catch (deleteError: unknown) { setError(toUserMessage(deleteError)); }
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
  const selectedAssetId = stage === "image" ? selectedShot?.selectedImageAssetId : selectedShot?.selectedVideoAssetId;
  const stageCandidateLinks = stageLinks.filter((link) => Boolean(link.task?.outputAssetIds.length));
  const shotCandidates = useMemo<ShotWorkspaceCandidate[]>(() => {
    const result = new Map<string, ShotWorkspaceCandidate>();
    for (const asset of [...stageCandidates, ...manualAssets]) {
      const link = stageCandidateLinks.find((candidateLink) => candidateLink.task?.outputAssetIds.includes(asset.id));
      const taskStatus = link?.task?.status;
      const status = selectedAssetId === asset.id
        ? "selected"
        : taskStatus === "FAILED"
          ? "failed"
          : taskStatus === "RUNNING" || taskStatus === "COLLECTING"
            ? "generating"
            : taskStatus === "QUEUED" || taskStatus === "CREATED" || taskStatus === "PREPARING"
              ? "queued"
              : "ready";
      if (!result.has(asset.id)) {
        result.set(asset.id, {
          asset,
          status,
          taskId: link?.task?.id ?? link?.taskId,
          createdAt: link?.createdAt,
          error: link?.task?.error?.message,
          fromLinkedTask: Boolean(link),
        });
      }
    }
    return [...result.values()];
  }, [manualAssets, selectedAssetId, stageCandidateLinks, stageCandidates]);
  const previewAsset = assets.find((asset) => asset.id === previewAssetId)
    ?? assets.find((asset) => asset.id === selectedAssetId)
    ?? shotCandidates[0]?.asset;
  const inspectorReferences = currentReferences.map((assetId, index) => ({
    assetId,
    ordinal: index,
    asset: assets.find((asset) => asset.id === assetId),
    label: `@图片${index + 1}`,
  }));
  const referenceAnchorOptions = referenceAnchors.map((anchor) => ({
    id: anchor.id,
    name: anchor.name,
    kind: anchor.kind,
    usable: anchor.usable,
    assets: anchor.assets.flatMap((item) => item.asset ? [item.asset] : []),
  }));
  const promptLibraryOptions = promptEntries.map((entry) => ({
    id: entry.id,
    name: entry.name,
    versionCount: entry.versions.length,
  }));
  const canGenerate = Boolean(currentDraft)
    && !(stage === "video" && ((!ref2vaMode && !selectedShot?.selectedImageAssetId) || Boolean(referenceValidation)));
  const treeShotFilter = useCallback((shot: ShotView) => {
    const query = shotListControls.query.trim().toLocaleLowerCase();
    if (query && !`${shot.name}\n${shot.promptText}`.toLocaleLowerCase().includes(query)) return false;
    if (shotListControls.status !== "ALL" && deriveShotStatus(shot) !== shotListControls.status) return false;
    if (shotListControls.sceneId === "UNASSIGNED") return !shotSceneIds[shot.id];
    return shotListControls.sceneId === "ALL" || shotSceneIds[shot.id] === shotListControls.sceneId;
  }, [shotListControls, shotSceneIds]);

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

  function selectWorkspaceSelection(selection: WorkspaceSelection) {
    setWorkspaceSelection(selection);
    if (selection.type !== "shot") return;
    setSelectedShotId(selection.shotId);
    onShotSelected?.(selection.shotId);
  }

  function openStructureManagement(context: WorkspaceSelection) {
    setStructureManagementOpen(true);
    setWorkspaceSelection(context);
    setNotice("已打开结构管理；Series / Episode / Scene 的新增、重命名、排序和归档仍由原有管理面板执行。");
  }

  function handleStructureCreate(target: ProjectStructureCreateTarget, context: WorkspaceSelection) {
    if (target === "shot") {
      void addShot();
      return;
    }
    openStructureManagement(context);
  }

  const productionSurface = (
    <div className="shot-production-surfaces" data-surface="production">
      <ProductionBatchRunbookPanel
        projectId={projectId}
        runbook={productionBatchRunbook}
        onRefresh={reload}
        onStartBatch={async (batchId) => { await startProductionQueue(projectId, batchId); await reload(); }}
        onOpenProductionQueue={onOpenProductionQueue ? () => onOpenProductionQueue() : undefined}
        onNavigateToEpisode={(episodeId) => selectWorkspaceSelection({ type: "episode", episodeId })}
        onNavigateToScene={(sceneId) => selectWorkspaceSelection({ type: "scene", sceneId })}
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
          selectWorkspaceSelection({ type: "shot", shotId: shotIds[0] });
        }}
      />
    </div>
  );

  const reviewSurface = (
    <div className="shot-review-surface" data-surface="review">
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
    </div>
  );
  const contextSurface = shotContextSurface(mode, workspaceSelection.type);
  const showWorkspaceFeedback = contextSurface !== "shot";

  if (loading) return <section className="workspace-panel shot-workspace"><p className="project-loading">正在加载镜头制作...</p></section>;

  return (
    <section className="workspace-panel shot-workspace" aria-busy={busy} data-studio-mode={mode}>
      {mode === "creation" && bulkImportOpen && <ShotBulkImportPanel
        projectId={projectId}
        onImported={async () => { await reload(); setBulkImportOpen(false); }}
        onCancel={() => setBulkImportOpen(false)}
      />}
      <div className="shot-production-layout">
        <div className="shot-structure-column">
          <ProjectStructureTree
            project={{ id: projectId, name: projectName ?? projectId }}
            tree={productionStructure}
            shots={shots}
            selectedSelection={workspaceSelection}
            onSelectSelection={selectWorkspaceSelection}
            onCreate={handleStructureCreate}
            openManagement={openStructureManagement}
            shotFilter={treeShotFilter}
            headerActions={<div className="shot-structure-more">
              <button type="button" className="shot-structure-more-button" aria-label="更多结构操作" aria-haspopup="menu" aria-expanded={structureMenuOpen} onClick={() => setStructureMenuOpen((open) => !open)}>⋯</button>
              {structureMenuOpen && <div className="shot-structure-more-menu" role="menu" aria-label="结构操作菜单">
                {mode === "creation" && <button type="button" role="menuitem" onClick={() => { setBulkImportOpen((open) => !open); setStructureMenuOpen(false); }}>{bulkImportOpen ? "收起批量导入" : "批量导入"}</button>}
                {mode !== "review" && <button type="button" role="menuitem" onClick={() => { openStructureManagement(workspaceSelection); setStructureMenuOpen(false); }}>结构管理</button>}
                <button type="button" role="menuitem" onClick={() => { void exportManifest(); setStructureMenuOpen(false); }} disabled={busy}>导出清单</button>
                <button type="button" role="menuitem" onClick={() => { void reload(); setStructureMenuOpen(false); }} disabled={busy}>刷新</button>
              </div>}
            </div>}
          />
          <section className="shot-structure-filter" aria-label="镜头搜索和筛选">
            <div className="shot-structure-filter-heading"><strong>镜头定位</strong><span>{shotList.filteredCount} / {shots.length}</span></div>
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
            {shotList.isFiltered && <div className="shot-search-results" aria-label="镜头搜索结果">
              {shotList.pageShots.map((item) => <button key={item.id} type="button" className={item.id === selectedShotId ? "shot-search-result shot-search-result-active" : "shot-search-result"} onClick={() => selectWorkspaceSelection({ type: "shot", shotId: item.id })}><span>{String(item.ordinal + 1).padStart(2, "0")}</span><strong>{item.name}</strong><small>{shotStatusLabels[deriveShotStatus(item)]}</small></button>)}
              {!shotList.pageShots.length && <span className="empty-state">没有匹配的镜头。</span>}
            </div>}
          </section>
        </div>
        <div className="shot-production-context">
          {contextSurface !== "shot" && <div className="shot-context-heading">
            <div>
              {mode !== "creation" && <span className="section-label">{mode === "production" ? "生产" : "审核"}</span>}
              <h2>{workspaceSelection.type === "scene" ? "场景工作区" : workspaceSelection.type === "episode" ? "Episode 工作区" : workspaceSelection.type === "series" ? "Series 工作区" : "项目工作区"}</h2>
              {mode === "production" && <p>Runbook 与项目批量管线集中在生产模式。</p>}
              {mode === "review" && <p>集中处理候选确认、失败重试和人工审核。</p>}
            </div>
            <span className="shot-context-selection">{workspaceSelection.type === "project" ? (projectName ?? projectId) : "已选结构节点"}</span>
          </div>}
          {contextSurface === "review" ? reviewSurface : contextSurface === "production" ? productionSurface : contextSurface === "shot" ? (
            <ShotCreationWorkspace
              projectId={projectId}
              shot={selectedShot}
              name={name}
              onNameChange={setName}
              stage={stage}
              onStageChange={setStage}
              candidates={shotCandidates}
              selectedAssetId={selectedAssetId}
              previewAsset={previewAsset}
              onCandidateSelect={(candidate) => setPreviewAssetId(candidate.asset.id)}
              onCandidateConfirm={(assetId, fromLinkedTask) => void selectResult(assetId, fromLinkedTask ?? false)}
              onOpenTask={onOpenTask}
              history={stageLinks}
              onRetry={(link) => retryShot(selectedShot?.id ?? "", link.stage)}
              onDeleteShot={() => void removeShot()}
              onCreateShot={() => void addShot()}
              onCopyPrompt={(prompt) => void navigator.clipboard?.writeText(prompt).then(() => setNotice("Prompt 已复制。"))}
              workspaceTab={shotWorkspaceTab}
              onWorkspaceTabChange={setShotWorkspaceTab}
              inspectorTab={inspectorTab}
              onInspectorTabChange={setInspectorTab}
              currentDraft={currentDraft}
              currentRecipe={currentRecipe}
              stageRecipes={stageRecipes}
              onRecipeChange={changeStageRecipe}
              onScalarChange={changeScalar}
              busy={busy}
              canGenerate={canGenerate}
              onGenerate={generate}
              configDirty={dirtyStages.has(stage)}
              onSave={save}
              references={inspectorReferences}
              availableReferences={imageAssets}
              referenceAnchors={referenceAnchorOptions}
              selectedAnchorId={selectedAnchorId}
              onAnchorChange={setSelectedAnchorId}
              keyframeAsset={selectedShot?.selectedImageAssetId ? assets.find((asset) => asset.id === selectedShot.selectedImageAssetId) : undefined}
              onReferenceAdd={(assetId) => setReferences((current) => ({
                ...current,
                [stage]: addOrderedReference(current[stage], assetId, stage === "video" ? ref2vaImageField?.maxItems : undefined),
              }))}
              onReferenceRemove={(assetId) => setReferences((current) => ({ ...current, [stage]: removeOrderedReference(current[stage], assetId) }))}
              onReferenceMove={(index, delta) => setReferences((current) => ({ ...current, [stage]: moveOrderedReference(current[stage], index, delta) }))}
              onApplyAnchor={(mode) => void applyReferenceAnchor(mode)}
              onSaveReferences={() => void replaceReferences()}
              promptText={promptText}
              onPromptChange={(text) => { setPromptText(text); setPromptProvenance(undefined); setSelectedPromptId(""); }}
              promptLibrary={promptLibraryOptions}
              selectedPromptId={selectedPromptId}
              onPromptSelect={setSelectedPromptId}
              onLoadPrompt={loadPrompt}
              promptProvenance={promptProvenance}
              promptPreview={promptText}
              promptTemplate={selectedPromptEntry?.kind === "prompt" && selectedPromptVersion && isPromptTemplateText(selectedPromptVersion.text) ? <PromptTemplatePanel
                projectId={projectId}
                projectName={projectName}
                projectDescription={projectDescription}
                stage={stage}
                entry={selectedPromptEntry}
                version={selectedPromptVersion}
                shot={selectedShot!}
                structureContext={workspaceSceneContext}
                referenceAnchors={referenceAnchors}
                onApplied={() => void reload()}
                disabled={busy}
              /> : undefined}
              onPreviewPrompt={() => setNotice("Prompt 预览使用当前编辑框内容；保存镜头后才会写入快照。")}
              onApplyPrompt={() => setNotice("当前 Prompt 预览已应用到编辑框；点击保存镜头写入快照。")}
              notice={notice}
              error={error}
            />
          ) : contextSurface === "project" ? (
            <section className="shot-context-empty" data-surface="creation-project">
              <strong>从项目结构开始</strong>
              <p>选择 Series、Episode、Scene 或 Shot，当前工作区会只显示对应的制作上下文。</p>
            </section>
          ) : contextSurface === "series" ? (
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
              onNavigateToEpisode={(episodeId) => selectWorkspaceSelection({ type: "episode", episodeId })}
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
          ) : contextSurface === "episode" ? (
            <EpisodeProductionPanel
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
              onNavigateToScene={(sceneId) => selectWorkspaceSelection({ type: "scene", sceneId })}
            />
          ) : (
            <SceneProductionPanel
              projectId={projectId}
              sceneOptions={sceneFilterOptions.filter((option) => option.value !== "ALL" && option.value !== "UNASSIGNED")}
              currentSceneId={workspaceSceneId}
              currentShot={selectedShot}
              promptEntries={promptEntries}
              referenceAnchors={referenceAnchors}
              initialPresets={batchWorkflowPresets}
              onRefresh={reload}
              onNotice={(message) => setNotice(message)}
              onNavigateToReview={(reviewStage) => setStage(reviewStage)}
            />
          )}
        </div>
      </div>
      {mode !== "review" && structureManagementOpen && <section className="shot-structure-management-panel">
        <div className="shot-secondary-heading"><div><span className="section-label">Structure management</span><h3>结构与批量管理</h3></div><button type="button" className="quiet-button" onClick={() => setStructureManagementOpen(false)}>收起管理面板</button></div>
        <ProductionStructurePanel
        projectId={projectId}
        tree={productionStructure}
        shots={shots}
        selectedShotId={selectedShotId}
        onSelectShot={(shotId) => selectWorkspaceSelection({ type: "shot", shotId })}
        onChanged={setProductionStructure}
        onError={(message) => setError(message || undefined)}
      />
      </section>}
      {mode !== "production" && <ProductionQueueDrawer
        runbook={productionBatchRunbook}
        onStart={async (batchId) => { await startProductionQueue(projectId, batchId); await reload(); }}
        onPause={async (batchId) => { await pauseProductionQueue(projectId, batchId); await reload(); }}
        onOpen={onOpenProductionQueue ? () => onOpenProductionQueue() : undefined}
      />}
      {showWorkspaceFeedback && notice && <p className="studio-notice">{notice}</p>}
      {showWorkspaceFeedback && error && <p className="error-message">{error}</p>}
    </section>
  );
}

export function buildShotContextPath(
  tree: ProductionStructureTree,
  selection: WorkspaceSelection,
  shots: readonly ShotView[] = [],
): ShotContextPathItem[] {
  switch (selection.type) {
    case "project":
      return [];
    case "series": {
      const series = orderedSeries(tree).find((item) => item.id === selection.seriesId);
      return series ? [{ type: "series", id: series.id, label: seriesLabel(series.ordinal, series.name) }] : [];
    }
    case "episode": {
      for (const series of orderedSeries(tree)) {
        const episode = orderedEpisodes(series).find((item) => item.id === selection.episodeId);
        if (episode) {
          return [
            { type: "series", id: series.id, label: seriesLabel(series.ordinal, series.name) },
            { type: "episode", id: episode.id, label: episodeLabel(episode.ordinal, episode.name) },
          ];
        }
      }
      return [];
    }
    case "scene": {
      const parent = findProductionSceneParent(tree, selection.sceneId);
      return parent ? structurePath(parent.series, parent.episode, parent.scene) : [];
    }
    case "shot": {
      const sceneId = shotSceneIndex(tree)[selection.shotId];
      const parent = sceneId ? findProductionSceneParent(tree, sceneId) : undefined;
      const shot = shots.find((item) => item.id === selection.shotId);
      const shotItem: ShotContextPathItem = { type: "shot", id: selection.shotId, label: shot?.name ?? `Shot ${selection.shotId}` };
      return parent ? [...structurePath(parent.series, parent.episode, parent.scene), shotItem] : [shotItem];
    }
  }
}

function selectionForContextPathItem(item: ShotContextPathItem): WorkspaceSelection {
  switch (item.type) {
    case "series":
      return { type: "series", seriesId: item.id };
    case "episode":
      return { type: "episode", episodeId: item.id };
    case "scene":
      return { type: "scene", sceneId: item.id };
    case "shot":
      return { type: "shot", shotId: item.id };
  }
}

function structurePath(
  series: ProductionStructureTree["series"][number],
  episode: ProductionStructureTree["series"][number]["episodes"][number],
  scene: ProductionStructureTree["series"][number]["episodes"][number]["scenes"][number],
): ShotContextPathItem[] {
  return [
    { type: "series", id: series.id, label: seriesLabel(series.ordinal, series.name) },
    { type: "episode", id: episode.id, label: episodeLabel(episode.ordinal, episode.name) },
    { type: "scene", id: scene.id, label: sceneLabel(scene.ordinal, scene.name) },
  ];
}

function seriesLabel(ordinal: number, name: string): string {
  return `系列 ${String(ordinal + 1).padStart(2, "0")} · ${name}`;
}

function episodeLabel(ordinal: number, name: string): string {
  return `第 ${String(ordinal + 1).padStart(2, "0")} 集 · ${name}`;
}

function sceneLabel(ordinal: number, name: string): string {
  return `Scene ${String(ordinal + 1).padStart(2, "0")} · ${name}`;
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

function isScalarField(field: RecipeField): field is Extract<RecipeField, { type: "integer" | "number" | "seed" }> {
  return field.type === "integer" || field.type === "number" || field.type === "seed";
}

function defaultScalarValues(recipe: RecipeViewModel): ShotInputValues {
  return Object.fromEntries(recipe.fields.filter(isScalarField).map((field) => {
    if (field.type === "integer") return [field.key, field.default === undefined ? { type: "integer", value: 0 } : { type: "integer", value: field.default }];
    if (field.type === "number") return [field.key, field.default === undefined ? { type: "number", value: 0 } : { type: "number", value: field.default }];
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

function emptyRunbook(projectId: string): ProductionBatchRunbookView {
  return { projectId, rows: [] };
}
