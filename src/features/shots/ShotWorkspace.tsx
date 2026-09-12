import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
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
  getProductionBatchReviewProductivity,
  getProjectWorkflowConfig,
  getProductionQueue,
  getSeriesProductionPlan,
  listPromptLibrary,
  listProductionStructure,
  listReferenceAnchors,
  listBatchWorkflowPresets,
  listRecentAssets,
  listShots,
  openProductionReviewOutputFolder,
  pickProductionPackageRoot,
  requeueProductionQueueItemByItem,
  revealProductionReviewAsset,
  replaceShotReferences,
  selectShotResult,
  prepareSeriesProduction,
  setShotStageConfig,
  startProductionQueue,
  updateShot,
} from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { DraftValue, RecipeField, RecipeViewModel } from "../../types/generation";
import type { ProjectWorkflowConfigView } from "../../types/projectWorkflow";
import type { ProjectCommandCenterCollectionFilter } from "../../types/projectCommandCenter";
import type { PromptEntryView } from "../../types/prompt";
import type { ReferenceAnchorView } from "../../types/referenceAnchor";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ProductionBatchRunbookView } from "../../types/productionBatchRunbook";
import type { SeriesPresetApplyRequest } from "../../types/seriesProduction";
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
import { SceneProductionPanel } from "./SceneProductionPanel";
import { EpisodeProductionPanel } from "./EpisodeProductionPanel";
import { SeriesProductionPanel } from "./SeriesProductionPanel";
import { ProductionBatchRunbookPanel } from "../production/ProductionBatchRunbookPanel";
import { ProductionPackageWorkspace } from "../production/ProductionPackageWorkspace";
import {
  MultiPackageProductionBoard,
} from "../production/MultiPackageProductionBoard";
import { ProductionQueueDrawer } from "../production/ProductionQueueDrawer";
import { ProductionMonitor as ProductionMonitorComponent } from "../production/ProductionMonitor";
import { ProductionReviewInbox } from "../production/ProductionReviewInbox";
import type { ProjectCommandCenterNavigationRequest } from "../projects/ProjectCommandCenter";
import type { ProductionMonitorProps } from "../production/ProductionMonitor";
import { ProductionAssetPreview } from "../studio/ProductionAssetPreview";
import { ProjectStructureTree, type ProjectStructureCreateTarget } from "./ProjectStructureTree";
import { ShotCreationWorkspace, type ShotCreationWorkspaceTab, type ShotWorkspaceCandidate } from "./ShotCreationWorkspace";
import {
  buildShotProductionReadModel,
  type ShotProductionContext,
  type ShotProductionStepId,
} from "./shotProductionState";
import { useShotWorkspaceSelection } from "./hooks/useShotWorkspaceSelection";
import { useShotQueueController, type ProductionQueueSnapshot } from "./hooks/useShotQueueController";
import { useShotProductionMonitor } from "./hooks/useShotProductionMonitor";
import { useShotMultiPackageController } from "./hooks/useShotMultiPackageController";
import { useShotTaskEvents } from "./hooks/useShotTaskEvents";
import {
  buildLocalDeliveryManifest,
  firstFinishedMonitorAsset,
  monitorCandidateFor,
  monitorReadModelFor,
  safeManifestPart,
} from "./shotProductionMonitorModel";
import { ScopeConsistencyWorkspace, type ScopeConsistencyWorkspaceProps } from "./ScopeConsistencyWorkspace";
import type { ConsistencyScopeOption, ConsistencyScopeRef } from "../../types/consistencyBindings";
import type { ShotInspectorTab } from "./ShotInspector";
import {
  appendAnchorReferences,
  replaceWithAnchorReferences,
} from "./referenceAnchorApply";
import {
  buildShotListView,
  shotListControlsForNavigation,
  updateShotListControls,
  type ShotListControls,
} from "./shotListQuery";
import { EMPTY_PRODUCTION_STRUCTURE, findProductionSceneParent, orderedEpisodes, orderedSeries, productionSceneOptions, shotSceneIndex } from "./productionStructureState";
import {
  h3FamilyForWorkflowId,
  h3QualityProfileForWorkflowId,
  MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
} from "../runtime/productRuntimeScope";
import {
  resolveShotStageRecipe,
  shotStageRecipeCompatibility,
  validateShotReferenceImages,
  type ShotVideoInputMode,
} from "../runtime/shotWorkflowCompatibility";
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

type ProductionModeTab = "package" | "project" | "multi-package";

export interface ProductionModeTabsProps {
  packagePanel: ReactNode;
  projectProductionPanel: ReactNode;
  multiPackagePanel?: ReactNode;
  activeTab?: ProductionModeTab;
  onActiveTabChange?: (tab: ProductionModeTab) => void;
}

export function ProductionModeTabs({ packagePanel, projectProductionPanel, multiPackagePanel, activeTab: controlledActiveTab, onActiveTabChange }: ProductionModeTabsProps) {
  const [uncontrolledActiveTab, setUncontrolledActiveTab] = useState<ProductionModeTab>("package");
  const activeTab = controlledActiveTab ?? uncontrolledActiveTab;
  const idPrefix = useId();
  const packageTabId = `${idPrefix}-production-package-tab`;
  const projectTabId = `${idPrefix}-project-production-tab`;
  const multiPackageTabId = `${idPrefix}-multi-package-production-tab`;
  const packagePanelId = `${idPrefix}-production-package-panel`;
  const projectPanelId = `${idPrefix}-project-production-panel`;
  const multiPackagePanelId = `${idPrefix}-multi-package-production-panel`;
  const selectTab = (tab: ProductionModeTab) => {
    onActiveTabChange?.(tab);
    if (controlledActiveTab === undefined) setUncontrolledActiveTab(tab);
  };

  return (
    <div className="shot-production-mode-tabs" data-surface="production" data-active-tab={activeTab}>
      <div className="shot-production-mode-tablist" role="tablist" aria-label="生产模式工作区">
        <button
          type="button"
          id={packageTabId}
          className="shot-production-mode-tab"
          role="tab"
          aria-selected={activeTab === "package"}
          aria-controls={packagePanelId}
          tabIndex={activeTab === "package" ? 0 : -1}
          onClick={() => selectTab("package")}
        >
          生产包
        </button>
        {multiPackagePanel !== undefined && (
          <button
            type="button"
            id={multiPackageTabId}
            className="shot-production-mode-tab"
            role="tab"
            aria-selected={activeTab === "multi-package"}
            aria-controls={multiPackagePanelId}
            tabIndex={activeTab === "multi-package" ? 0 : -1}
            onClick={() => selectTab("multi-package")}
          >
            批量生产包
          </button>
        )}
        <button
          type="button"
          id={projectTabId}
          className="shot-production-mode-tab"
          role="tab"
          aria-selected={activeTab === "project"}
          aria-controls={projectPanelId}
          tabIndex={activeTab === "project" ? 0 : -1}
          onClick={() => selectTab("project")}
        >
          项目生产
        </button>
      </div>

      <section
        id={packagePanelId}
        className="shot-production-mode-tabpanel"
        role="tabpanel"
        aria-labelledby={packageTabId}
        aria-hidden={activeTab !== "package"}
        hidden={activeTab !== "package"}
        data-tab-panel="production-package"
      >
        {packagePanel}
      </section>
      {multiPackagePanel !== undefined && <section
        id={multiPackagePanelId}
        className="shot-production-mode-tabpanel"
        role="tabpanel"
        aria-labelledby={multiPackageTabId}
        aria-hidden={activeTab !== "multi-package"}
        hidden={activeTab !== "multi-package"}
        data-tab-panel="multi-package-production"
      >
        {multiPackagePanel}
      </section>}
      <section
        id={projectPanelId}
        className="shot-production-mode-tabpanel"
        role="tabpanel"
        aria-labelledby={projectTabId}
        aria-hidden={activeTab !== "project"}
        hidden={activeTab !== "project"}
        data-tab-panel="project-production"
      >
        {projectProductionPanel}
      </section>
    </div>
  );
}

interface Props {
  projectId: string;
  projectName?: string;
  catalog: RecipeViewModel[];
  initialSelectedShotId?: string;
  initialCollectionFilter?: ProjectCommandCenterCollectionFilter;
  mode?: ShotWorkspaceMode;
  onShotSelected?: (shotId?: string) => void;
  onContextPathChange?: (path: ShotContextPathItem[]) => void;
  contextPathTarget?: ShotContextPathItem;
  onOpenAsset?: (assetId: string) => void;
  onOpenTask?: (taskId: string) => void;
  onNavigate?: (request: ProjectCommandCenterNavigationRequest) => void;
  focusProductionBatchId?: string;
  focusProductionReviewItemId?: string;
  focusProductionStage?: ShotStage;
  onOpenProductionQueue?: () => void;
  consistencyWorkspace?: Omit<ScopeConsistencyWorkspaceProps, "projectId" | "scope" | "scopeOptions" | "onScopeChange"> & {
    scopeOptions?: ConsistencyScopeOption[];
    onScopeChange?: (scope: ConsistencyScopeRef) => void;
  };
}

type StageDraft = {
  workflowVersionId: string;
  recipeId: string;
  values: ShotInputValues;
};

const emptyStageDrafts: Partial<Record<ShotStage, StageDraft>> = {};

const ProductionMonitor = ProductionMonitorComponent;

export { buildLocalDeliveryManifest } from "./shotProductionMonitorModel";

export function ShotWorkspace({ projectId, projectName, catalog, initialSelectedShotId, initialCollectionFilter, mode = "creation", onShotSelected, onContextPathChange, contextPathTarget, onOpenAsset, onOpenTask, onNavigate, focusProductionBatchId, focusProductionReviewItemId, focusProductionStage, onOpenProductionQueue, consistencyWorkspace }: Props) {
  const [shots, setShots] = useState<ShotView[]>([]);
  const {
    selectedShotId,
    workspaceSelection,
    selectWorkspaceSelection,
    selectShot,
    reconcileSelectedShot,
  } = useShotWorkspaceSelection({ projectId, initialSelectedShotId, onShotSelected });
  const [stage, setStage] = useState<ShotStage>(focusProductionStage ?? (initialCollectionFilter?.kind === "shots" ? initialCollectionFilter.stage : undefined) ?? "image");
  const [stageDrafts, setStageDrafts] = useState<Partial<Record<ShotStage, StageDraft>>>(emptyStageDrafts);
  const [dirtyStages, setDirtyStages] = useState<Set<ShotStage>>(new Set());
  const [references, setReferences] = useState<Record<ShotStage, string[]>>({ image: [], video: [] });
  const [assets, setAssets] = useState<AssetView[]>([]);
  const [referenceAnchors, setReferenceAnchors] = useState<ReferenceAnchorView[]>([]);
  const [projectWorkflowConfig, setProjectWorkflowConfig] = useState<ProjectWorkflowConfigView>();
  const [projectWorkflowConfigError, setProjectWorkflowConfigError] = useState<string>();
  const [productionStructure, setProductionStructure] = useState<ProductionStructureTree>(() => EMPTY_PRODUCTION_STRUCTURE(projectId));
  const [productionBatchRunbook, setProductionBatchRunbook] = useState<ProductionBatchRunbookView>(() => emptyRunbook(projectId));
  const [productionPackageFolderPath, setProductionPackageFolderPath] = useState<string | null>(null);
  const [productionPackageWorkspaceKey, setProductionPackageWorkspaceKey] = useState(0);
  const [productionModeTab, setProductionModeTab] = useState<ProductionModeTab>("package");
  const [batchWorkflowPresets, setBatchWorkflowPresets] = useState<BatchWorkflowPreset[]>([]);
  const [selectedAnchorId, setSelectedAnchorId] = useState("");
  const [promptEntries, setPromptEntries] = useState<PromptEntryView[]>([]);
  const [selectedPromptId, setSelectedPromptId] = useState("");
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
  const [shotListControls, setShotListControls] = useState<ShotListControls>(() => shotListControlsForNavigation(initialCollectionFilter));
  const reloadGeneration = useRef(0);
  const monitorRefreshRef = useRef<((batchId: string) => Promise<void>) | undefined>(undefined);
  const monitorFocusRef = useRef<((batchId: string) => void) | undefined>(undefined);

  useEffect(() => {
    let active = true;
    setProjectWorkflowConfig(undefined);
    setProjectWorkflowConfigError(undefined);
    void getProjectWorkflowConfig(projectId)
      .then((config) => {
        if (!active) return;
        if (!config) {
          setProjectWorkflowConfigError("项目工作流配置读取为空，已阻止正式 Shot 生产。");
          return;
        }
        setProjectWorkflowConfig(config);
      })
      .catch((loadError: unknown) => {
        if (active) setProjectWorkflowConfigError(`项目工作流配置加载失败：${toUserMessage(loadError)}`);
      });
    return () => {
      active = false;
    };
  }, [projectId]);

  const selectedShot = shots.find((shot) => shot.id === selectedShotId);
  const shotSceneIds = useMemo(() => shotSceneIndex(productionStructure), [productionStructure]);
  const sceneFilterOptions = useMemo(() => productionSceneOptions(productionStructure), [productionStructure]);
  const workspaceSceneId = workspaceSelection.type === "scene"
    ? workspaceSelection.sceneId
    : workspaceSelection.type === "shot"
      ? shotSceneIds[workspaceSelection.shotId]
      : undefined;
  const contextPath = useMemo(
    () => buildShotContextPath(productionStructure, workspaceSelection, shots),
    [productionStructure, shots, workspaceSelection],
  );
  const shotList = useMemo(() => buildShotListView(shots, shotListControls, shotSceneIds), [shots, shotListControls, shotSceneIds]);
  const currentDraft = stageDrafts[stage];
  const productCatalog = catalog;
  const stageRecipes = useMemo(
    () => productCatalog.filter((recipe) => shotStageRecipeCompatibility(recipe, stage).compatible),
    [productCatalog, stage],
  );
  const currentRecipe = productCatalog.find(
    (recipe) =>
      recipe.workflowVersionId === currentDraft?.workflowVersionId &&
      recipe.recipeId === currentDraft?.recipeId,
  );
  const currentCompatibility = currentRecipe ? shotStageRecipeCompatibility(currentRecipe, stage) : undefined;
  const currentStageConfig = selectedShot?.stageConfigs.find((config) => config.stage === stage);
  const currentProjectDefault = projectDefaultForStage(projectWorkflowConfig, stage);
  const stageResolution = useMemo(
    () => resolveShotStageRecipe(
      productCatalog,
      stage,
      currentStageConfig,
      currentProjectDefault,
      preferredStageRecipe(productCatalog, stage),
    ),
    [currentProjectDefault, currentStageConfig, productCatalog, stage],
  );
  const productionReadModel = useMemo(() => {
    if (!selectedShot) return undefined;

    const stageContext = (nextStage: ShotStage): ShotProductionContext[ShotStage] => {
      const stageConfig = selectedShot.stageConfigs.find((config) => config.stage === nextStage);
      const projectDefault = projectDefaultForStage(projectWorkflowConfig, nextStage);
      const resolution = resolveShotStageRecipe(
        productCatalog,
        nextStage,
        stageConfig,
        projectDefault,
        preferredStageRecipe(productCatalog, nextStage),
      );
      const recipe = resolution.recipe;
      const compatibility = recipe ? shotStageRecipeCompatibility(recipe, nextStage) : undefined;
      const referenceField = recipe?.fields.find(
        (field): field is Extract<RecipeField, { type: "image" | "images" }> => field.type === "image" || field.type === "images",
      );
      const referenceCount = orderedShotReferences(selectedShot, nextStage).length;
      const available = Boolean(projectWorkflowConfig && recipe && !resolution.blocked);
      const configured = Boolean(stageConfig || projectDefault);
      return {
        available,
        configured,
        referenceRequired: nextStage === "video"
          ? compatibility?.videoInputMode === "REFERENCE_IMAGES"
          : Boolean(referenceField?.required),
        referenceMinimum: referenceField?.type === "images"
          ? Math.max(isRef2vaRecipe(recipe) ? 2 : 1, referenceField.minItems)
          : referenceField?.type === "image"
            ? 1
            : undefined,
        referenceCount,
        videoInputMode: nextStage === "video" ? compatibility?.videoInputMode : undefined,
      };
    };

    return buildShotProductionReadModel(selectedShot, {
      image: stageContext("image"),
      video: stageContext("video"),
    });
  }, [productCatalog, projectWorkflowConfig, selectedShot]);
  const currentReferences = references[stage] ?? [];
  const selectedAnchor = referenceAnchors.find((anchor) => anchor.id === selectedAnchorId);
  const ref2vaMode = stage === "video" && isRef2vaRecipe(currentRecipe);
  const ref2vaImageField = ref2vaMode ? referenceImagesField(currentRecipe) : undefined;
  const videoInputMode: ShotVideoInputMode | undefined = stage === "video" ? currentCompatibility?.videoInputMode : undefined;
  const videoReferenceField = currentRecipe?.fields.find((field): field is Extract<RecipeField, { type: "images" }> => field.type === "images");
  const referenceValidation = videoInputMode === "REFERENCE_IMAGES"
    ? ref2vaMode
      ? validateRef2vaReferences(ref2vaImageField, currentReferences)
      : validateShotReferenceImages(videoReferenceField, currentReferences)
    : undefined;
  const imageAssets = assets.filter(isImageAsset);
  const videoAssets = assets.filter(isVideoAsset);

  const navigateProductionStep = useCallback((stepId: ShotProductionStepId) => {
    if (stepId === "prompt") {
      setShotWorkspaceTab("generate");
      setInspectorTab("prompt");
      return;
    }
    if (stepId === "references") {
      setShotWorkspaceTab("references");
      setInspectorTab("references");
      return;
    }
    if (stepId === "image" || stepId === "image-review") {
      setStage("image");
      setShotWorkspaceTab("generate");
      return;
    }
    if (stepId === "video" || stepId === "video-review") {
      setStage("video");
      setShotWorkspaceTab("generate");
    }
  }, []);

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
    selectShot(next.id);
  }, [selectShot]);

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
      reconcileSelectedShot(nextShots.map((shot) => shot.id));
    } catch (loadError: unknown) {
      if (generation !== reloadGeneration.current) return;
      setError(toUserMessage(loadError));
    } finally {
      if (generation === reloadGeneration.current) setLoading(false);
    }
  }, [projectId, reconcileSelectedShot]);

  useEffect(() => {
    if (!initialSelectedShotId || loading || shots.some((shot) => shot.id === initialSelectedShotId)) return;
    setError("目标镜头不存在或已不可用。");
  }, [initialSelectedShotId, loading, shots]);

  const reloadProductionQueuesRef = useRef<((throwOnError?: boolean) => Promise<ProductionQueueSnapshot | undefined>) | undefined>(undefined);
  const reloadProductionQueues = useCallback(
    (throwOnError = false) => reloadProductionQueuesRef.current?.(throwOnError) ?? Promise.resolve(undefined),
    [],
  );

  const multiPackageController = useShotMultiPackageController({
    projectId,
    enabled: mode === "production",
    reloadProductionQueues,
    onError: setError,
    onNotice: setNotice,
  });
  const {
    rootPath: multiPackageRootPath,
    boardPackages: multiPackageBoardPackages,
    isDiscovering: multiPackageDiscovering,
    isCreating: multiPackageCreating,
    inspectProgress: multiPackageProgress,
    refresh: refreshMultiPackageBoard,
    chooseRoot: chooseMultiPackageRoot,
    createSelected: createMultiPackageBatches,
    reinspect: reinspectMultiPackage,
    findDiscoveredPackage,
    bestBatchIdForPackage,
  } = multiPackageController;

  const refreshMonitorBridge = useCallback((batchId: string) => {
    return monitorRefreshRef.current?.(batchId) ?? Promise.resolve();
  }, []);
  const focusMonitorBridge = useCallback((batchId: string) => {
    monitorFocusRef.current?.(batchId);
  }, []);
  const refreshQueuesForTaskEvents = useCallback(async () => {
    await reloadProductionQueues();
  }, [reloadProductionQueues]);
  const queueController = useShotQueueController({
    projectId,
    enabled: mode === "production",
    reloadWorkspace: reload,
    refreshProductionMonitor: refreshMonitorBridge,
    onError: setError,
    onNotice: setNotice,
    onFocusBatch: focusMonitorBridge,
    onOpenProductionQueue,
  });
  reloadProductionQueuesRef.current = queueController.reloadProductionQueues;
  const {
    queues: productionQueues,
    overview: productionQueueOverview,
    expanded: productionQueueExpanded,
    setExpanded: setProductionQueueExpanded,
    sequential: sequentialBatchStart,
    focusedBatchId: focusedProductionBatchId,
    createdBatchIds: recentlyCreatedProductionBatchIds,
    selectedBatchId: selectedProductionBatchId,
    focusBatch: focusProductionQueueBatch,
    openQueue: openProductionQueue,
    startBatch: startProductionBatch,
    cancelQueuedStart: cancelQueuedSequentialBatch,
    cancelSequentialStart: cancelSequentialBatchStart,
    resumeSequentialStart: resumeSequentialBatchStart,
    onMonitorBatchChanged,
    pauseBatch: pauseProductionBatch,
    requeueItem: requeueProductionMonitorItemFromQueue,
  } = queueController;

  useEffect(() => {
    if (mode !== "production" || !focusProductionBatchId) return;
    let active = true;
    void reloadProductionQueues(true).then((snapshot) => {
      if (!active) return;
      if (!snapshot?.queues.some((queue) => queue.id === focusProductionBatchId)) {
        setError("目标生产批次不存在或已不可用。");
        return;
      }
      focusProductionQueueBatch(focusProductionBatchId);
    }).catch((loadError: unknown) => {
      if (active) setError(toUserMessage(loadError));
    });
    return () => {
      active = false;
    };
  }, [focusProductionBatchId, focusProductionQueueBatch, mode, reloadProductionQueues]);

  useEffect(() => {
    if (mode === "review" && focusProductionStage) setStage(focusProductionStage);
  }, [focusProductionStage, mode]);

  const monitorController = useShotProductionMonitor({
    projectId,
    enabled: mode === "production",
    selectedBatchId: selectedProductionBatchId,
  });
  monitorRefreshRef.current = monitorController.refresh;
  monitorFocusRef.current = monitorController.focusBatch;
  const {
    batch: productionMonitorBatch,
    review: productionMonitorReview,
    loading: productionMonitorLoading,
    error: productionMonitorError,
    previewAsset: monitorPreviewAsset,
    setPreviewAsset: setMonitorPreviewAsset,
    clearError: clearProductionMonitorError,
    setError: setProductionMonitorError,
    refresh: refreshProductionMonitor,
    getCurrentBatchId: getMonitorBatchId,
  } = monitorController;

  useEffect(() => {
    if (mode !== "production") return;
    onMonitorBatchChanged();
  }, [mode, onMonitorBatchChanged, productionMonitorBatch]);

  useShotTaskEvents({
    enabled: mode === "production",
    projectId,
    productionModeTab,
    onRefreshQueues: refreshQueuesForTaskEvents,
    onRefreshMultiPackage: refreshMultiPackageBoard,
    getMonitorBatchId,
    onRefreshMonitor: refreshProductionMonitor,
  });

  useEffect(() => { void reload(); }, [reload]);

  useEffect(() => {
    if (mode !== "production" || productionModeTab !== "multi-package") return;
    void refreshMultiPackageBoard();
  }, [mode, productionModeTab, refreshMultiPackageBoard]);

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
    if (initialCollectionFilter?.kind !== "shots") return;
    setShotListControls(shotListControlsForNavigation(initialCollectionFilter));
    if (initialCollectionFilter.stage) setStage(initialCollectionFilter.stage);
  }, [initialCollectionFilter]);

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
      const projectDefault = projectDefaultForStage(projectWorkflowConfig, nextStage);
      const resolution = resolveShotStageRecipe(
        productCatalog,
        nextStage,
        config,
        projectDefault,
        preferredStageRecipe(productCatalog, nextStage),
      );
      if (config) {
        nextDrafts[nextStage] = {
          workflowVersionId: config.workflowVersionId,
          recipeId: config.recipeId,
          values: config.scalarValues as ShotInputValues,
        };
      } else if (resolution.recipe) {
        nextDrafts[nextStage] = {
          workflowVersionId: resolution.recipe.workflowVersionId,
          recipeId: resolution.recipe.recipeId,
          values: defaultScalarValues(resolution.recipe),
        };
      } else if (projectDefault && resolution.blocked) {
        nextDrafts[nextStage] = {
          workflowVersionId: projectDefault.workflowVersionId,
          recipeId: projectDefault.recipeId,
          values: {},
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
  }, [productCatalog, projectWorkflowConfig, selectedShot]);

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
      setNotice("镜头设置已保存；提示词已保存为当前快照。后续提示词库更新不会自动改动此镜头。");
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
      reconcileSelectedShot(remaining.map((shot) => shot.id));
    } catch (deleteError: unknown) { setError(toUserMessage(deleteError)); }
    finally { setBusy(false); }
  }

  async function replaceReferences() {
    if (!selectedShot) return;
    if (videoInputMode === "REFERENCE_IMAGES" && referenceValidation) {
      setError(referenceValidation);
      return;
    }
    setBusy(true); setError(undefined);
    try {
      const next = await replaceShotReferences({ projectId, shotId: selectedShot.id, stage, assetIds: currentReferences });
      applyShot(next);
      setNotice("参考素材已保存为关系；不会复制素材文件。");
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
    const nextValidation = videoInputMode === "REFERENCE_IMAGES"
      ? ref2vaMode
        ? validateRef2vaReferences(ref2vaImageField, nextReferences)
        : validateShotReferenceImages(videoReferenceField, nextReferences)
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
      setNotice(`${mode === "append" ? "已追加" : "已替换为"}参考锚点“${selectedAnchor.name}”；镜头仅保存素材 ID。`);
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
    if (!projectWorkflowConfig || projectWorkflowConfigError || stageResolution.blocked || !currentRecipe || !currentCompatibility?.compatible) {
      setError(projectWorkflowConfigError ?? stageResolution.reason ?? "当前阶段工作流不可用于正式 Shot 生产。");
      return;
    }
    if (stage === "video") {
      const inputError = videoInputMode === "SINGLE_IMAGE"
        ? selectedShot.selectedImageAssetId ? undefined : "请先选择关键帧图片。"
        : videoInputMode === "REFERENCE_IMAGES"
          ? referenceValidation
          : videoInputMode === "UNSUPPORTED"
            ? currentCompatibility.reason ?? "当前视频 Recipe 的媒体输入无法由 Shot 提供。"
            : undefined;
      if (inputError) {
        setError(inputError);
        return;
      }
    }
    setBusy(true); setError(undefined); setNotice(undefined);
    try {
      if (videoInputMode === "REFERENCE_IMAGES" && !sameReferenceOrder(currentReferences, orderedShotReferences(selectedShot, "video"))) {
        const next = await replaceShotReferences({ projectId, shotId: selectedShot.id, stage: "video", assetIds: currentReferences });
        applyShot(next);
      }
      const persistedConfig = selectedShot.stageConfigs.find((config) => config.stage === stage);
      const draftChanged = !persistedConfig
        || dirtyStages.has(stage)
        || persistedConfig.workflowVersionId !== currentDraft.workflowVersionId
        || persistedConfig.recipeId !== currentDraft.recipeId;
      if (draftChanged) {
        const next = await setShotStageConfig({
          projectId,
          shotId: selectedShot.id,
          stage,
          workflowVersionId: currentDraft.workflowVersionId,
          recipeId: currentDraft.recipeId,
          values: currentDraft.values,
        });
        applyShot(next);
      }
      const task = await generateShot({ projectId, shotId: selectedShot.id, stage, values: currentDraft.values });
      setNotice(`任务 ${task.id} 已创建；镜头状态由任务和候选素材派生。不会自动跳过候选选择。`);
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
    setPromptText(version.text);
    setPromptProvenance({ entryId: entry.id, versionId: version.id });
    setNotice(`已载入提示词库「${entry.name}」的 v${version.version}；之后编辑会清除来源标记。`);
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
  const videoInputReady = videoInputMode === "TEXT_ONLY"
    ? true
    : videoInputMode === "SINGLE_IMAGE"
      ? Boolean(selectedShot?.selectedImageAssetId)
      : videoInputMode === "REFERENCE_IMAGES"
        ? !referenceValidation
        : false;
  const stageConfigurationError = projectWorkflowConfigError
    ?? (projectWorkflowConfig && stageResolution.blocked ? stageResolution.reason : undefined);
  const canGenerate = Boolean(
    projectWorkflowConfig
      && !projectWorkflowConfigError
      && currentDraft
      && currentRecipe
      && currentCompatibility?.compatible
      && !stageResolution.blocked
      && (stage !== "video" || videoInputReady),
  );
  const treeShotFilter = useCallback((shot: ShotView) => {
    const query = shotListControls.query.trim().toLocaleLowerCase();
    if (query && !`${shot.name}\n${shot.promptText}`.toLocaleLowerCase().includes(query)) return false;
    if (shotListControls.status !== "ALL" && deriveShotStatus(shot) !== shotListControls.status) return false;
    if (shotListControls.sceneId === "UNASSIGNED") return !shotSceneIds[shot.id];
    return shotListControls.sceneId === "ALL" || shotSceneIds[shot.id] === shotListControls.sceneId;
  }, [shotListControls, shotSceneIds]);

  async function configureBulkStage(nextStage: ShotStage, shotIds: string[]) {
    if (projectWorkflowConfigError) throw new Error(projectWorkflowConfigError);
    if (!projectWorkflowConfig) throw new Error("项目工作流配置尚未加载，请稍后重试。");
    const projectDefault = projectDefaultForStage(projectWorkflowConfig, nextStage);
    const resolution = resolveShotStageRecipe(
      productCatalog,
      nextStage,
      undefined,
      projectDefault,
      preferredStageRecipe(productCatalog, nextStage),
    );
    if (resolution.blocked || !resolution.recipe) {
      throw new Error(resolution.reason ?? `当前没有可用的${nextStage === "image" ? "图片" : "视频"}配方。`);
    }
    const recipe = resolution.recipe;
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

  function openStructureManagement(context: WorkspaceSelection) {
    setStructureManagementOpen(true);
    selectWorkspaceSelection(context);
    setNotice("已打开结构管理；系列 / 集 / 场景的新增、重命名、排序和归档仍由原有管理面板执行。");
  }

  function handleStructureCreate(target: ProjectStructureCreateTarget, context: WorkspaceSelection) {
    if (target === "shot") {
      void addShot();
      return;
    }
    openStructureManagement(context);
  }

  const openProductionMonitorBatch = useCallback((batchId: string) => {
    focusProductionQueueBatch(batchId);
    if (typeof document === "undefined" || document.visibilityState !== "hidden") {
      void refreshProductionMonitor(batchId);
    }
  }, [focusProductionQueueBatch, refreshProductionMonitor]);

  const requeueProductionMonitorItem = useCallback(async (itemId: string) => {
    clearProductionMonitorError();
    try {
      await requeueProductionMonitorItemFromQueue(itemId);
    } catch (requeueError: unknown) {
      setProductionMonitorError(toUserMessage(requeueError));
    }
  }, [requeueProductionMonitorItemFromQueue]);

  const productionMonitorReadModel = useMemo(
    () => monitorReadModelFor(productionMonitorBatch, productionMonitorReview, projectId),
    [productionMonitorBatch, productionMonitorReview, projectId],
  );
  const finishedMonitorAsset = useMemo(
    () => firstFinishedMonitorAsset(productionMonitorReview),
    [productionMonitorReview],
  );
  const openMonitorAssetPreviewForItem = useCallback((itemId: string, assetId: string) => {
    const item = productionMonitorReview?.items.find((candidate) => candidate.itemId === itemId);
    const asset = item?.outputAssets.find((candidate) => candidate.id === assetId);
    if (asset && isVideoAsset(asset)) setMonitorPreviewAsset(asset);
  }, [productionMonitorReview]);
  const revealMonitorAssetLocation = useCallback(async (itemId: string, filePath?: string) => {
    const batchId = selectedProductionBatchId;
    const item = productionMonitorReview?.items.find((candidate) => candidate.itemId === itemId);
    const candidate = item?.candidateAssets.find((asset) => filePath && asset.localPath === filePath)
      ?? (item ? monitorCandidateFor(item, true) : undefined);
    if (!batchId || !candidate?.localPath) {
      setProductionMonitorError("该成品没有可用的数据库文件位置。");
      return;
    }
    try {
      await revealProductionReviewAsset({ projectId, batchId, itemId, assetId: candidate.assetId });
    } catch (openError: unknown) {
      setProductionMonitorError(toUserMessage(openError));
    }
  }, [projectId, productionMonitorReview, selectedProductionBatchId]);
  const openMonitorOutputFolder = useCallback(async () => {
    const batchId = selectedProductionBatchId;
    const finished = finishedMonitorAsset;
    if (!batchId || !finished?.localPath) {
      setProductionMonitorError("没有可打开的数据库成品目录。");
      return;
    }
    try {
      await openProductionReviewOutputFolder({
        projectId,
        batchId,
        itemId: finished.itemId,
        assetId: finished.assetId,
      });
    } catch (openError: unknown) {
      setProductionMonitorError(toUserMessage(openError));
    }
  }, [finishedMonitorAsset, projectId, selectedProductionBatchId]);
  const exportLocalDeliveryManifest = useCallback(async () => {
    const batchId = selectedProductionBatchId;
    if (!batchId || typeof document === "undefined" || typeof URL === "undefined" || typeof URL.createObjectURL !== "function") {
      setProductionMonitorError("当前批次交付数据尚未准备好。");
      return;
    }
    try {
      const [detail, review] = await Promise.all([
        getProductionQueue(projectId, batchId),
        getProductionBatchReviewProductivity(projectId, batchId),
      ]);
      if (getMonitorBatchId() !== batchId || selectedProductionBatchId !== batchId) {
        setProductionMonitorError("当前监控批次已切换，请重新打开当前批次后再导出成品清单。");
        return;
      }
      const batch = monitorReadModelFor(detail, review, projectId);
      const manifest = batch ? buildLocalDeliveryManifest(batch, review, batchId) : undefined;
      if (!manifest) {
        setProductionMonitorError("当前监控批次数据不一致，请重新打开当前批次后再导出成品清单。");
        return;
      }
      const blob = new Blob([JSON.stringify(manifest, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `LOCAL_DELIVERY_MANIFEST_${safeManifestPart(manifest.batchId)}.json`;
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      setNotice("LOCAL_DELIVERY_MANIFEST 已导出；仅包含数据库成品索引。");
    } catch (exportError: unknown) {
      setProductionMonitorError(toUserMessage(exportError));
    }
  }, [getMonitorBatchId, projectId, selectedProductionBatchId]);
  const selectNextProductionPackage = useCallback(async () => {
    try {
      const nextPath = await pickProductionPackageRoot();
      if (nextPath) {
        setProductionPackageFolderPath(nextPath);
        setNotice("已选择下一个生产包，正在检查；不会自动创建或开始批次。");
      }
    } catch (pickError: unknown) {
      setProductionMonitorError(toUserMessage(pickError));
    }
  }, []);
  const openBoardPackage = useCallback((packageKey: string) => {
    const discoveredPackage = findDiscoveredPackage(packageKey);
    if (!discoveredPackage) return;
    setProductionPackageFolderPath(discoveredPackage.packageRoot);
    setProductionPackageWorkspaceKey((current) => current + 1);
    setProductionModeTab("package");
    setNotice(`已打开「${discoveredPackage.relativePath || discoveredPackage.packageRoot}」单生产包工作区；不会自动创建或开始批次。`);
  }, [findDiscoveredPackage]);
  const openMultiPackageBatch = useCallback((_packageKey: string, batchIds: string[]) => {
    const batchId = bestBatchIdForPackage(_packageKey, batchIds);
    if (!batchId) return;
    focusProductionQueueBatch(batchId);
    void openProductionQueue()
      .then(() => openProductionMonitorBatch(batchId))
      .catch((openError: unknown) => setError(`打开生产批次失败：${toUserMessage(openError)}`));
  }, [bestBatchIdForPackage, focusProductionQueueBatch, openProductionMonitorBatch, openProductionQueue]);
  const multiPackageBoardPollingEnabled = productionModeTab === "multi-package"
    && multiPackageBoardPackages.some((item) => item.status === "CREATED" || item.status === "RUNNING");
  const monitorProps: ProductionMonitorProps = {
    batch: productionMonitorReadModel,
    readModel: productionMonitorReadModel,
    onRetry: requeueProductionMonitorItem,
    onRetryItem: requeueProductionMonitorItem,
    onPlay: openMonitorAssetPreviewForItem,
    onOpenFileLocation: revealMonitorAssetLocation,
    onViewAllFinishedProducts: () => {
      if (selectedProductionBatchId) openProductionMonitorBatch(selectedProductionBatchId);
    },
    onOpenFinishedProductsFolder: finishedMonitorAsset?.localPath ? openMonitorOutputFolder : undefined,
    onExportManifest: productionMonitorReview ? exportLocalDeliveryManifest : undefined,
    onSelectNextProductionPackage: selectNextProductionPackage,
  };

  const productionSurface = (
    <ProductionModeTabs
      activeTab={productionModeTab}
      onActiveTabChange={setProductionModeTab}
      packagePanel={(
        <ProductionPackageWorkspace
          key={productionPackageWorkspaceKey}
          projectId={projectId}
          folderPath={productionPackageFolderPath}
          onFolderPathChange={setProductionPackageFolderPath}
          onChooseFolder={pickProductionPackageRoot}
          onOpenProductionQueue={openProductionQueue}
        />
      )}
      projectProductionPanel={(
        <div className="shot-production-surfaces" data-surface="project-production">
          <ProductionBatchRunbookPanel
            projectId={projectId}
            runbook={productionBatchRunbook}
            onRefresh={reload}
            onStartBatch={startProductionBatch}
            onOpenProductionQueue={onOpenProductionQueue ? () => { void openProductionQueue(); } : undefined}
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
            onOpenProductionQueue={onOpenProductionQueue ? () => { void openProductionQueue(); } : undefined}
            busy={busy}
            onOpenReview={(reviewStage, shotIds) => {
              if (busy || !shotIds.length) return;
              setStage(reviewStage);
              selectWorkspaceSelection({ type: "shot", shotId: shotIds[0] });
            }}
          />
        </div>
      )}
      multiPackagePanel={(
        <MultiPackageProductionBoard
          packages={multiPackageBoardPackages}
          rootPath={multiPackageRootPath}
          isDiscovering={multiPackageDiscovering}
          inspectProgress={multiPackageProgress}
          isCreating={multiPackageCreating}
          onChooseRoot={() => void chooseMultiPackageRoot()}
          onOpenPackage={openBoardPackage}
          onHandleWarning={openBoardPackage}
          onViewIssues={openBoardPackage}
          onReinspect={reinspectMultiPackage}
          onOpenBatch={openMultiPackageBatch}
          onCreateSelected={createMultiPackageBatches}
          onRefresh={refreshMultiPackageBoard}
          pollingEnabled={multiPackageBoardPollingEnabled}
        />
      )}
    />
  );

  const reviewSurface = (
    <div className="shot-review-surface" data-surface="review">
      {mode === "review" && initialCollectionFilter?.kind === "review" && <ProductionReviewInbox projectId={projectId} mode="workspace" onNavigate={onNavigate} />}
      {!(mode === "review" && initialCollectionFilter?.kind === "review") && <ShotBatchReviewBoard
        projectId={projectId}
        shots={initialCollectionFilter?.kind === "shots" ? shotList.filteredShots : shots}
        assets={assets}
        stage={stage}
        busy={busy}
        onAssetsLoaded={(loaded) => setAssets((current) => [...current, ...loaded.filter((asset) => !current.some((item) => item.id === asset.id))])}
        onSelect={(shotId, reviewStage, assetId, fromLinkedTask) => void selectBatchResult(shotId, reviewStage, assetId, fromLinkedTask)}
        onRetry={(shotId, reviewStage) => void retryShot(shotId, reviewStage)}
        onOpenTask={onOpenTask}
        reviewBatchId={mode === "review" ? focusProductionBatchId : undefined}
        initialReviewItemId={mode === "review" ? focusProductionReviewItemId : undefined}
        onOpenProductionQueue={onOpenProductionQueue}
      />}
    </div>
  );
  const contextSurface = shotContextSurface(mode, workspaceSelection.type);
  const showWorkspaceFeedback = contextSurface !== "shot";
  const consistencyScope = consistencyScopeForSelection(workspaceSelection, projectId, projectName, productionStructure, shots);
  const showConsistencyScope = mode === "creation" && Boolean(consistencyWorkspace) && Boolean(consistencyScope) && contextSurface !== "shot";

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
              <h2>{workspaceSelection.type === "scene" ? "场景工作区" : workspaceSelection.type === "episode" ? "集工作区" : workspaceSelection.type === "series" ? "系列工作区" : "项目工作区"}</h2>
              {mode === "production" && <p>运行手册与项目批量流程集中在生产模式。</p>}
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
              onOpenAsset={onOpenAsset}
              onOpenTask={onOpenTask}
              history={stageLinks}
              onRetry={(link) => retryShot(selectedShot?.id ?? "", link.stage)}
              onDeleteShot={() => void removeShot()}
              onCreateShot={() => void addShot()}
              onCopyPrompt={(prompt) => void navigator.clipboard?.writeText(prompt).then(() => setNotice("提示词已复制。"))}
              workspaceTab={shotWorkspaceTab}
              onWorkspaceTabChange={setShotWorkspaceTab}
              production={productionReadModel}
              onProductionNavigate={navigateProductionStep}
              consistency={consistencyWorkspace && selectedShot ? {
                ...consistencyWorkspace,
                projectId,
                scope: { scopeType: "SHOT", scopeId: selectedShot.id, scopeName: selectedShot.name },
                scopeOptions: [],
                onScopeChange: (nextScope) => consistencyWorkspace.onScopeChange?.(nextScope),
              } : undefined}
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
                [stage]: addOrderedReference(current[stage], assetId, stage === "video" ? videoReferenceField?.maxItems : undefined),
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
              onPreviewPrompt={() => setNotice("提示词预览使用当前编辑框内容；保存镜头后才会写入快照。")}
              onApplyPrompt={() => setNotice("当前提示词预览已应用到编辑框；点击保存镜头写入快照。")}
              notice={notice}
              error={error ?? stageConfigurationError}
            />
          ) : showConsistencyScope && consistencyScope && consistencyWorkspace ? (
            <ScopeConsistencyWorkspace
              {...consistencyWorkspace}
              projectId={projectId}
              scope={consistencyScope}
              scopeOptions={consistencyWorkspace.scopeOptions}
              onScopeChange={(nextScope) => {
                consistencyWorkspace.onScopeChange?.(nextScope);
                const nextSelection = selectionForConsistencyScope(nextScope);
                if (nextSelection) selectWorkspaceSelection(nextSelection);
              }}
            />
          ) : contextSurface === "project" ? (
            <section className="shot-context-empty" data-surface="creation-project">
              <strong>从项目结构开始</strong>
              <p>选择系列、集、场景或镜头，当前工作区会只显示对应的制作上下文。</p>
            </section>
          ) : contextSurface === "series" ? (
            <SeriesProductionPanel
              projectId={projectId}
              tree={productionStructure}
              shots={shots}
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
            />
          ) : contextSurface === "episode" ? (
            <EpisodeProductionPanel
              projectId={projectId}
              tree={productionStructure}
              shots={shots}
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
              initialPresets={batchWorkflowPresets}
              onRefresh={reload}
              onNotice={(message) => setNotice(message)}
              onNavigateToReview={(reviewStage) => setStage(reviewStage)}
              onOpenProductionQueue={onOpenProductionQueue}
            />
          )}
        </div>
      </div>
      {mode !== "review" && structureManagementOpen && <section className="shot-structure-management-panel">
        <div className="shot-secondary-heading"><div><span className="section-label">结构管理</span><h3>结构与批量管理</h3></div><button type="button" className="quiet-button" onClick={() => setStructureManagementOpen(false)}>收起管理面板</button></div>
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
      <ProductionQueueDrawer
        overview={mode === "production" ? productionQueueOverview : undefined}
        queues={mode === "production" ? productionQueues : undefined}
        runbook={productionBatchRunbook}
        expanded={mode === "production" ? productionQueueExpanded : undefined}
        onToggle={mode === "production" ? setProductionQueueExpanded : undefined}
        focusBatchId={mode === "production" ? focusedProductionBatchId : undefined}
        createdBatchIds={mode === "production" ? recentlyCreatedProductionBatchIds : undefined}
        queuedStartBatchIds={sequentialBatchStart.queuedBatchIds}
        currentSequentialBatchId={sequentialBatchStart.currentBatchId}
        sequentialStartStatus={sequentialBatchStart.status}
        sequentialPauseReason={sequentialBatchStart.pauseReason}
        sequentialCanResume={sequentialBatchStart.canResume}
        onStart={startProductionBatch}
        onCancelQueuedStart={cancelQueuedSequentialBatch}
        onResumeSequentialStart={resumeSequentialBatchStart}
        onCancelSequentialStart={cancelSequentialBatchStart}
        onPause={pauseProductionBatch}
        onOpen={mode === "production"
          ? (batchId) => openProductionMonitorBatch(batchId)
          : onOpenProductionQueue ? () => onOpenProductionQueue() : undefined}
      />
      {mode === "production" && (
        <>
          {productionMonitorLoading && !productionMonitorReadModel && <p className="project-loading" role="status">正在加载生产监控...</p>}
          {productionMonitorError && <p className="error-message" role="alert">{productionMonitorError}</p>}
          <ProductionMonitor {...monitorProps} />
          {monitorPreviewAsset && (
            <ProductionAssetPreview
              projectId={projectId}
              asset={monitorPreviewAsset}
              onClose={() => setMonitorPreviewAsset(undefined)}
              onOpenTask={onOpenTask}
            />
          )}
        </>
      )}
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
      const shotItem: ShotContextPathItem = { type: "shot", id: selection.shotId, label: shot?.name ?? `镜头 ${selection.shotId}` };
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
  return `场景 ${String(ordinal + 1).padStart(2, "0")} · ${name}`;
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
  if (!field) return "REF2VA 配方缺少多图参考输入";
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

function projectDefaultForStage(
  config: ProjectWorkflowConfigView | undefined,
  stage: ShotStage,
): ProjectWorkflowConfigView["imageDefault"] | ProjectWorkflowConfigView["videoDefault"] | undefined {
  return stage === "image" ? config?.imageDefault : config?.videoDefault;
}

function preferredStageRecipe(catalog: RecipeViewModel[], stage: ShotStage): RecipeViewModel | undefined {
  const compatible = catalog.filter((item) => shotStageRecipeCompatibility(item, stage).compatible);
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

function consistencyScopeForSelection(
  selection: WorkspaceSelection,
  projectId: string,
  projectName: string | undefined,
  tree: ProductionStructureTree,
  shots: readonly ShotView[],
): ConsistencyScopeRef | undefined {
  switch (selection.type) {
    case "project":
      return { scopeType: "PROJECT", scopeId: projectId, scopeName: projectName ?? projectId };
    case "series": {
      const series = orderedSeries(tree).find((item) => item.id === selection.seriesId);
      return series ? { scopeType: "SERIES", scopeId: series.id, scopeName: series.name } : undefined;
    }
    case "episode": {
      for (const series of orderedSeries(tree)) {
        const episode = orderedEpisodes(series).find((item) => item.id === selection.episodeId);
        if (episode) return { scopeType: "EPISODE", scopeId: episode.id, scopeName: episode.name };
      }
      return undefined;
    }
    case "scene": {
      const parent = findProductionSceneParent(tree, selection.sceneId);
      return parent ? { scopeType: "SCENE", scopeId: parent.scene.id, scopeName: parent.scene.name } : undefined;
    }
    case "shot": {
      const shot = shots.find((item) => item.id === selection.shotId);
      return shot ? { scopeType: "SHOT", scopeId: shot.id, scopeName: shot.name } : undefined;
    }
  }
}

function selectionForConsistencyScope(scope: ConsistencyScopeRef): WorkspaceSelection | undefined {
  switch (scope.scopeType) {
    case "PROJECT": return { type: "project", projectId: scope.scopeId };
    case "SERIES": return { type: "series", seriesId: scope.scopeId };
    case "EPISODE": return { type: "episode", episodeId: scope.scopeId };
    case "SCENE": return { type: "scene", sceneId: scope.scopeId };
    case "SHOT": return { type: "shot", shotId: scope.scopeId };
  }
}
