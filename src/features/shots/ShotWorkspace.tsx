import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import {
  createShot,
  bulkAssignShotPrompt,
  bulkSetShotStageConfig,
  createProductionPackageBatches,
  discoverProductionPackages,
  deleteShot,
  exportProjectManifest,
  generateShot,
  getAsset,
  getAssetMediaUrl,
  getShot,
  getProductionBatchRunbook,
  getProductionBatchReviewProductivity,
  getProductionQueue,
  getProductionQueueOverview,
  getSeriesProductionPlan,
  inspectProductionPackage,
  listPromptLibrary,
  listProductionQueues,
  listProductionPackageBindings,
  listProductionStructure,
  listReferenceAnchors,
  listBatchWorkflowPresets,
  listRecentAssets,
  listShots,
  openProductionReviewOutputFolder,
  pauseProductionQueue,
  pickProductionPackageRoot,
  requeueProductionQueueItem,
  requeueProductionQueueItemByItem,
  revealProductionReviewAsset,
  replaceShotReferences,
  selectShotResult,
  prepareSeriesProduction,
  previewPromptTemplateBulk,
  applyPromptTemplate,
  setShotStageConfig,
  startProductionQueue,
  updateShot,
} from "../../services/tauriClient";
import type { ProductionPackageCreateBatchesResult } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { DraftValue, RecipeField, RecipeViewModel } from "../../types/generation";
import type { PromptEntryView } from "../../types/prompt";
import type { ReferenceAnchorView } from "../../types/referenceAnchor";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ProductionBatchRunbookView } from "../../types/productionBatchRunbook";
import type { ProductionBatchSummary, ProductionQueueOverview } from "../../types/productionQueue";
import type { ProductionBatchDetail } from "../../types/productionQueue";
import type {
  ProductionPackageBatchBinding,
  ProductionPackageDiscoveryPackage,
  ProductionPackageInspectionResult,
} from "../../types/productionPackage";
import type {
  ProductionBatchReviewProductivity,
} from "../../services/tauriClient";
import { subscribeTaskUpdates } from "../../services/taskEvents";
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
import { ProductionPackageWorkspace } from "../production/ProductionPackageWorkspace";
import {
  MultiPackageProductionBoard,
  type MultiPackageBoardInspectProgress,
  type MultiPackageBoardPackage,
} from "../production/MultiPackageProductionBoard";
import { ProductionQueueDrawer } from "../production/ProductionQueueDrawer";
import { ProductionMonitor as ProductionMonitorComponent } from "../production/ProductionMonitor";
import type {
  ProductionMonitorBatchReadModel,
  ProductionMonitorItemReadModel,
  ProductionMonitorProps,
} from "../production/ProductionMonitor";
import { ProductionAssetPreview } from "../studio/ProductionAssetPreview";
import { ProjectStructureTree, type ProjectStructureCreateTarget } from "./ProjectStructureTree";
import { ShotCreationWorkspace, type ShotCreationWorkspaceTab, type ShotWorkspaceCandidate } from "./ShotCreationWorkspace";
import { ScopeConsistencyWorkspace, type ScopeConsistencyWorkspaceProps } from "./ScopeConsistencyWorkspace";
import type { ConsistencyScopeOption, ConsistencyScopeRef } from "../../types/consistencyBindings";
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
  projectDescription?: string | null;
  catalog: RecipeViewModel[];
  initialSelectedShotId?: string;
  mode?: ShotWorkspaceMode;
  onShotSelected?: (shotId?: string) => void;
  onContextPathChange?: (path: ShotContextPathItem[]) => void;
  contextPathTarget?: ShotContextPathItem;
  onOpenTask?: (taskId: string) => void;
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

interface ProductionQueueSnapshot {
  queues: ProductionBatchSummary[];
  overview: ProductionQueueOverview;
}

const ProductionMonitor = ProductionMonitorComponent;

function isTerminalProductionBatch(batch?: ProductionBatchDetail | null): boolean {
  if (!batch) return false;
  const terminalItemCount = batch.succeeded + batch.failed + batch.cancelled + batch.skipped;
  return Boolean(
    batch.archivedAt
      || batch.status === "COMPLETED"
      || ["FAILED", "CANCELLED", "CANCELED"].includes(batch.status as string)
      || (batch.total > 0 && terminalItemCount >= batch.total),
  );
}

function multiPackageBatchOpenPriority(batch?: ProductionBatchDetail): number {
  if (!batch) return 3;
  if (batch.status === "RUNNING" || batch.running > 0) return 0;
  if (batch.failed > 0) return 1;
  if (batch.status === "READY" || batch.status === "PAUSED" || batch.pending > 0) return 2;
  return 3;
}

function recentFirst<T extends { updatedAt?: string; createdAt?: string }>(left: T, right: T): number {
  const leftTime = Date.parse(left.updatedAt ?? left.createdAt ?? "") || 0;
  const rightTime = Date.parse(right.updatedAt ?? right.createdAt ?? "") || 0;
  return rightTime - leftTime;
}

export function selectDefaultProductionBatchId(
  queues: readonly ProductionBatchSummary[],
  focusedBatchId?: string,
): string | undefined {
  const available = queues.filter((queue) => !queue.archivedAt);
  const focused = focusedBatchId && available.some((queue) => queue.id === focusedBatchId)
    ? focusedBatchId
    : undefined;
  if (focused) return focused;
  return [...available].filter((queue) => queue.status === "RUNNING").sort(recentFirst)[0]?.id
    ?? [...available].sort(recentFirst)[0]?.id;
}

function monitorReadModelFor(
  batch: ProductionBatchDetail | undefined,
  review: ProductionBatchReviewProductivity | undefined,
  projectId: string,
): ProductionMonitorBatchReadModel | undefined {
  const source = batch ?? review?.batch;
  if (!source) return undefined;
  const reviewItems = new Map((review?.items ?? []).map((item) => [item.itemId, item]));
  const sourceItems = source.items.length
    ? source.items
    : (review?.items ?? []).map((item) => ({
      id: item.itemId,
      ordinal: item.ordinal,
      workflowVersionId: item.workflowVersionId,
      recipeId: item.recipeId,
      status: item.productionItemStatus as ProductionBatchDetail["items"][number]["status"],
      taskId: item.taskId,
      errorCode: undefined,
      errorMessage: undefined,
    }));
  const items: ProductionMonitorItemReadModel[] = sourceItems.map((item) => {
    const reviewItem = reviewItems.get(item.id);
    const outputAssets = reviewItem?.outputAssets ?? [];
    const candidates = reviewItem?.candidateAssets ?? [];
    const explicitlySelectedAsset = outputAssets.find((asset) => asset.id === reviewItem?.selectedAssetId);
    const selectedAsset = (explicitlySelectedAsset && isVideoAsset(explicitlySelectedAsset))
      ? explicitlySelectedAsset
      : outputAssets.find(isVideoAsset) ?? explicitlySelectedAsset ?? outputAssets[0];
    const candidateOutputs = candidates.map((candidate) => ({
      assetId: candidate.assetId,
      assetType: candidate.assetType,
      name: candidate.name,
      mimeType: candidate.mimeType,
      localPath: candidate.localPath,
      width: candidate.width,
      height: candidate.height,
      thumbnailAvailable: candidate.thumbnailAvailable,
      selected: candidate.selected,
      reviewResult: candidate.reviewResult,
      asset: outputAssets.find((asset) => asset.id === candidate.assetId),
    }));
    const candidateWithLocation = candidates.find((candidate) => candidate.assetId === selectedAsset?.id && candidate.localPath);
    const assetOutput = selectedAsset ? {
      ...selectedAsset,
      candidates: candidateOutputs,
    } : candidates.length ? { candidates: candidateOutputs } : undefined;
    return {
      id: item.id,
      ordinal: item.ordinal,
      status: item.status,
      name: reviewItem?.shotId ?? reviewItem?.promptText ?? item.id,
      promptText: reviewItem?.promptText ?? ("promptText" in item ? item.promptText : undefined),
      errorCode: item.errorCode,
      errorMessage: item.errorMessage,
      assetId: selectedAsset?.id ?? candidates[0]?.assetId,
      videoUrl: selectedAsset && (selectedAsset.assetType === "video" || selectedAsset.category === "generated_video")
        ? getAssetMediaUrl(projectId, selectedAsset.id, "video")
        : undefined,
      localPath: candidateWithLocation?.localPath,
      output: assetOutput,
      media: assetOutput,
      recordAvailable: Boolean(selectedAsset || candidates.length),
    };
  });
  return {
    id: source.id,
    name: source.name,
    status: source.status,
    total: source.total,
    pending: source.pending,
    running: source.running,
    succeeded: source.succeeded,
    failed: source.failed,
    cancelled: source.cancelled,
    skipped: source.skipped,
    items,
  };
}

type MonitorReviewItem = ProductionBatchReviewProductivity["items"][number];

function monitorCandidateFor(item: MonitorReviewItem, videoOnly = false) {
  const candidates = item.candidateAssets;
  return candidates.find((candidate) => candidate.assetId === item.selectedAssetId && (!videoOnly || isVideoCandidate(candidate)))
    ?? candidates.find((candidate) => !videoOnly || isVideoCandidate(candidate));
}

function isVideoCandidate(candidate: { assetType?: string; mimeType?: string }): boolean {
  return `${candidate.assetType ?? ""} ${candidate.mimeType ?? ""}`.toLowerCase().includes("video");
}

function firstFinishedMonitorAsset(review?: ProductionBatchReviewProductivity): { itemId: string; assetId: string; localPath?: string } | undefined {
  for (const item of review?.items ?? []) {
    if (item.productionItemStatus !== "SUCCEEDED") continue;
    const candidate = monitorCandidateFor(item, true);
    if (candidate) return { itemId: item.itemId, assetId: candidate.assetId, localPath: candidate.localPath };
  }
  return undefined;
}

function safeManifestPart(value: string): string {
  return value.replace(/[^a-zA-Z0-9._-]+/g, "_").replace(/^\.+|\.+$/g, "") || "batch";
}

export function buildLocalDeliveryManifest(
  batch: ProductionMonitorBatchReadModel,
  review: ProductionBatchReviewProductivity,
  expectedBatchId: string,
) {
  const batchId = String(batch.id ?? batch.batchId ?? "");
  const reviewBatchId = String(review.batch.id ?? "");
  const batchItems = batch.items ?? [];
  const reviewItems = review.items ?? [];
  const batchItemIds = new Set(batchItems.map((item) => String(item.id ?? item.itemId ?? "")));
  const reviewItemIds = new Set(reviewItems.map((item) => item.itemId));
  const itemsMatch = batchItems.length === reviewItems.length
    && batchItemIds.size === batchItems.length
    && reviewItemIds.size === reviewItems.length
    && [...batchItemIds].every((itemId) => reviewItemIds.has(itemId));

  if (
    !expectedBatchId
    || batchId !== expectedBatchId
    || reviewBatchId !== expectedBatchId
    || (batch.total !== undefined && review.total !== undefined && batch.total !== review.total)
    || !itemsMatch
  ) {
    return undefined;
  }

  const items = [...reviewItems]
    .sort((left, right) => left.ordinal - right.ordinal || left.itemId.localeCompare(right.itemId))
    .map((item) => {
      const candidate = monitorCandidateFor(item, true);
      return {
        externalId: item.shotId ?? item.itemId,
        itemId: item.itemId,
        status: item.productionItemStatus,
        videoAssetId: candidate?.assetId,
        videoPath: candidate?.localPath,
      };
    });

  return {
    manifestType: "LOCAL_DELIVERY_MANIFEST" as const,
    manifestVersion: 1 as const,
    batchId,
    batchName: batch.name ?? batch.batchName ?? review.batch.name,
    generatedAt: new Date().toISOString(),
    total: batch.total ?? review.total ?? items.length,
    succeeded: batch.succeeded ?? review.successCount,
    failed: batch.failed ?? review.failedCount,
    items,
  };
}

export function ShotWorkspace({ projectId, projectName, projectDescription, catalog, initialSelectedShotId, mode = "creation", onShotSelected, onContextPathChange, contextPathTarget, onOpenTask, onOpenProductionQueue, consistencyWorkspace }: Props) {
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
  const [productionQueues, setProductionQueues] = useState<ProductionBatchSummary[]>([]);
  const [productionQueueOverview, setProductionQueueOverview] = useState<ProductionQueueOverview>();
  const [productionQueueExpanded, setProductionQueueExpanded] = useState(false);
  const [focusedProductionBatchId, setFocusedProductionBatchId] = useState<string>();
  const [productionMonitorBatch, setProductionMonitorBatch] = useState<ProductionBatchDetail>();
  const [productionMonitorReview, setProductionMonitorReview] = useState<ProductionBatchReviewProductivity>();
  const [productionMonitorLoading, setProductionMonitorLoading] = useState(false);
  const [productionMonitorError, setProductionMonitorError] = useState<string>();
  const [monitorPreviewAsset, setMonitorPreviewAsset] = useState<AssetView>();
  const [productionPackageFolderPath, setProductionPackageFolderPath] = useState<string | null>(null);
  const [productionPackageWorkspaceKey, setProductionPackageWorkspaceKey] = useState(0);
  const [productionModeTab, setProductionModeTab] = useState<ProductionModeTab>("package");
  const [multiPackageRootPath, setMultiPackageRootPath] = useState<string | null>(null);
  const [multiPackagePackages, setMultiPackagePackages] = useState<ProductionPackageDiscoveryPackage[]>([]);
  const [multiPackageInspections, setMultiPackageInspections] = useState<Record<string, ProductionPackageInspectionResult>>({});
  const [multiPackageInspectionErrors, setMultiPackageInspectionErrors] = useState<Record<string, string>>({});
  const [multiPackageCreateMessages, setMultiPackageCreateMessages] = useState<Record<string, { status: "CREATE_FAILED" | "NOT_CREATED"; message: string }>>({});
  const [multiPackageBindings, setMultiPackageBindings] = useState<ProductionPackageBatchBinding[]>([]);
  const [multiPackageBatchDetails, setMultiPackageBatchDetails] = useState<Record<string, ProductionBatchDetail>>({});
  const [multiPackageDiscovering, setMultiPackageDiscovering] = useState(false);
  const [multiPackageCreating, setMultiPackageCreating] = useState(false);
  const [multiPackageProgress, setMultiPackageProgress] = useState<MultiPackageBoardInspectProgress>();
  const multiPackageRunId = useRef(0);
  const multiPackageRefreshInFlight = useRef(false);
  const multiPackageRefreshPending = useRef(false);
  const multiPackageMounted = useRef(true);
  const [recentlyCreatedProductionBatchIds, setRecentlyCreatedProductionBatchIds] = useState<string[]>([]);
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
  const productionMonitorRequest = useRef(false);
  const productionMonitorPendingBatch = useRef<string | undefined>(undefined);
  const productionMonitorMounted = useRef(true);
  const productionMonitorBatchRef = useRef<string | undefined>(undefined);

  useEffect(() => {
    multiPackageMounted.current = true;
    return () => {
      multiPackageMounted.current = false;
      multiPackageRunId.current += 1;
      multiPackageRefreshPending.current = false;
    };
  }, []);

  const selectedShot = shots.find((shot) => shot.id === selectedShotId);
  const selectedProductionBatchId = useMemo(
    () => selectDefaultProductionBatchId(productionQueues, focusedProductionBatchId),
    [focusedProductionBatchId, productionQueues],
  );
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

  const reloadProductionQueues = useCallback(async (throwOnError = false): Promise<ProductionQueueSnapshot | undefined> => {
    try {
      const [nextQueues, nextOverview] = await Promise.all([
        listProductionQueues(projectId),
        getProductionQueueOverview(projectId),
      ]);
      setProductionQueues(nextQueues);
      setProductionQueueOverview(nextOverview);
      return { queues: nextQueues, overview: nextOverview };
    } catch (queueError: unknown) {
      if (throwOnError) throw queueError;
      setError(toUserMessage(queueError));
      return undefined;
    }
  }, [projectId]);

  const refreshMultiPackageBoard = useCallback(async () => {
    if (!multiPackageMounted.current) return;
    if (multiPackageRefreshInFlight.current) {
      multiPackageRefreshPending.current = true;
      return;
    }
    multiPackageRefreshInFlight.current = true;
    try {
      const bindings = await listProductionPackageBindings(projectId);
      if (!multiPackageMounted.current) return;
      const nextDetails: Record<string, ProductionBatchDetail> = {};
      const batchIds = [...new Set(bindings.map((binding) => binding.batchId))];
      let detailError: unknown;
      for (const batchId of batchIds) {
        if (!multiPackageMounted.current) return;
        try {
          nextDetails[batchId] = await getProductionQueue(projectId, batchId);
          if (!multiPackageMounted.current) return;
        } catch (error: unknown) {
          detailError = error;
        }
      }
      if (!multiPackageMounted.current) return;
      setMultiPackageBindings(bindings);
      setMultiPackageBatchDetails(nextDetails);
      await reloadProductionQueues();
      if (detailError) setError(`多生产包看板刷新失败：${toUserMessage(detailError)}`);
    } catch (error: unknown) {
      if (multiPackageMounted.current) setError(`多生产包看板刷新失败：${toUserMessage(error)}`);
    } finally {
      multiPackageRefreshInFlight.current = false;
      if (multiPackageMounted.current && multiPackageRefreshPending.current) {
        multiPackageRefreshPending.current = false;
        void refreshMultiPackageBoard();
      }
    }
  }, [projectId, reloadProductionQueues]);

  const chooseMultiPackageRoot = useCallback(async () => {
    if (multiPackageDiscovering || multiPackageCreating) return;
    let runId: number | undefined;
    try {
      const pickedRoot = await pickProductionPackageRoot();
      if (!pickedRoot || !multiPackageMounted.current) return;
      runId = ++multiPackageRunId.current;
      setMultiPackageRootPath(pickedRoot);
      setMultiPackagePackages([]);
      setMultiPackageInspections({});
      setMultiPackageInspectionErrors({});
      setMultiPackageCreateMessages({});
      setMultiPackageProgress({ current: 0, total: 0 });
      setMultiPackageDiscovering(true);
      setError(undefined);
      const discovery = await discoverProductionPackages(pickedRoot);
      if (!multiPackageMounted.current || runId !== multiPackageRunId.current) return;
      setMultiPackageRootPath(discovery.rootPath);
      setMultiPackageProgress({ current: 0, total: discovery.packages.length });
      let readyCount = 0;
      let warningCount = 0;
      let blockedCount = 0;
      for (const [index, discoveredPackage] of discovery.packages.entries()) {
        if (!multiPackageMounted.current || runId !== multiPackageRunId.current) return;
        setMultiPackageProgress({
          current: index,
          total: discovery.packages.length,
          currentPackage: discoveredPackage.relativePath || discoveredPackage.packageRoot,
          readyCount,
          warningCount,
          blockedCount,
        });
        const packageKey = discoveredPackage.packageKey;
        try {
          const inspection = await inspectProductionPackage(projectId, discoveredPackage.packageRoot);
          if (!multiPackageMounted.current || runId !== multiPackageRunId.current) return;
          if (inspection.manifestSha256 !== discoveredPackage.manifestSha256) {
            throw new Error("production-package.json 在发现后发生变化，请重新选择根目录。");
          }
          setMultiPackageInspections((current) => ({ ...current, [packageKey]: inspection }));
          setMultiPackagePackages((current) => [...current, discoveredPackage]);
          readyCount += inspection.readyCount;
          warningCount += inspection.warningCount;
          blockedCount += inspection.blockedCount;
        } catch (inspectionError: unknown) {
          if (!multiPackageMounted.current || runId !== multiPackageRunId.current) return;
          setMultiPackageInspectionErrors((current) => ({
            ...current,
            [packageKey]: toUserMessage(inspectionError),
          }));
          setMultiPackagePackages((current) => [...current, discoveredPackage]);
          blockedCount += 1;
        }
        if (!multiPackageMounted.current || runId !== multiPackageRunId.current) return;
        setMultiPackageProgress({
          current: index + 1,
          total: discovery.packages.length,
          readyCount,
          warningCount,
          blockedCount,
        });
      }
      if (multiPackageMounted.current && runId === multiPackageRunId.current) await refreshMultiPackageBoard();
    } catch (discoveryError: unknown) {
      if (multiPackageMounted.current && (runId === undefined || runId === multiPackageRunId.current)) {
        setMultiPackagePackages([]);
        setMultiPackageProgress(undefined);
        setError(`发现生产包失败：${toUserMessage(discoveryError)}`);
      }
    } finally {
      if (multiPackageMounted.current && runId !== undefined && runId === multiPackageRunId.current) {
        setMultiPackageDiscovering(false);
        setMultiPackageProgress((current) => current ? { ...current, currentPackage: undefined } : current);
      }
    }
  }, [discoverProductionPackages, inspectProductionPackage, multiPackageCreating, multiPackageDiscovering, projectId, refreshMultiPackageBoard]);

  const createMultiPackageBatches = useCallback(async (packageKeys: string[]) => {
    if (multiPackageCreating || !multiPackageMounted.current) return;
    setMultiPackageCreating(true);
    setError(undefined);
    try {
      for (let index = 0; index < packageKeys.length; index += 1) {
        if (!multiPackageMounted.current) return;
        const packageKey = packageKeys[index];
        const discoveredPackage = multiPackagePackages.find((item) => item.packageKey === packageKey);
        try {
          if (!discoveredPackage) {
            throw new Error("该生产包尚未完成检查，请先重新检查。");
          }
          const inspection = await inspectProductionPackage(projectId, discoveredPackage.packageRoot);
          if (!multiPackageMounted.current) return;
          if (inspection.manifestSha256 !== discoveredPackage.manifestSha256) {
            throw new Error("production-package.json 在发现后发生变化，请重新选择根目录。");
          }
          setMultiPackageInspections((current) => ({ ...current, [packageKey]: inspection }));
          const safetyError = multiPackageInspectionSafetyError(inspection);
          if (safetyError) throw new Error(safetyError);
          setMultiPackageCreateMessages((current) => {
            const next = { ...current };
            delete next[packageKey];
            return next;
          });
          const boundItemIds = new Set(
            multiPackageBindings
              .filter((binding) => binding.packageKey === discoveredPackage.packageKey)
              .flatMap((binding) => binding.packageItemIds),
          );
          const selectedItemIds = inspection.items
            .filter((item) => item.status === "READY" && !boundItemIds.has(item.id))
            .map((item) => item.id);
          if (!selectedItemIds.length) continue;
          const result = await createProductionPackageBatches(inspection.inspectionId, selectedItemIds);
          if (!multiPackageMounted.current) return;
          await refreshMultiPackageBoard();
          if (!multiPackageMounted.current) return;
          if (result.status === "PARTIAL" || result.remainingCount > 0) {
            setNotice(`「${inspection.packageName}」已部分创建；请从剩余项目继续。`);
            setMultiPackageCreateMessages((current) => {
              const next = { ...current };
              for (const deferredKey of packageKeys.slice(index + 1)) {
                next[deferredKey] = {
                  status: "NOT_CREATED",
                  message: "未执行：前一个生产包仅部分创建；请先处理剩余项后再继续。",
                };
              }
              return next;
            });
            break;
          }
        } catch (packageError: unknown) {
          if (!multiPackageMounted.current) return;
          const message = packageError instanceof Error ? packageError.message : toUserMessage(packageError);
          setMultiPackageCreateMessages((current) => {
            const next = {
              ...current,
              [packageKey]: { status: "CREATE_FAILED" as const, message },
            };
            for (const deferredKey of packageKeys.slice(index + 1)) {
              next[deferredKey] = {
                status: "NOT_CREATED",
                message: "未执行：前一个生产包创建失败；可继续创建未创建或剩余项。",
              };
            }
            return next;
          });
          throw new Error(`「${discoveredPackage?.packageRoot ?? packageKey}」创建失败：${message}`);
        }
      }
    } finally {
      if (multiPackageMounted.current) {
        setMultiPackageCreating(false);
        await refreshMultiPackageBoard();
      }
    }
  }, [inspectProductionPackage, multiPackageBindings, multiPackageCreating, multiPackagePackages, projectId, refreshMultiPackageBoard]);

  const multiPackageBoardPackages = useMemo(
    () => multiPackagePackages.map((discoveredPackage) => {
      const packageKey = discoveredPackage.packageKey;
      return buildMultiPackageBoardPackage({
        discoveredPackage,
        inspection: multiPackageInspections[packageKey],
        inspectionError: multiPackageInspectionErrors[packageKey],
        bindings: multiPackageBindings,
        batchDetails: multiPackageBatchDetails,
        createMessage: multiPackageCreateMessages[packageKey],
      });
    }),
    [multiPackageBatchDetails, multiPackageBindings, multiPackageCreateMessages, multiPackageInspectionErrors, multiPackageInspections, multiPackagePackages],
  );

  const refreshProductionMonitor = useCallback(async (batchId: string) => {
    if (!productionMonitorMounted.current) return;
    if (productionMonitorRequest.current) {
      productionMonitorPendingBatch.current = batchId;
      return;
    }
    productionMonitorRequest.current = true;
    setProductionMonitorLoading(true);
    setProductionMonitorError(undefined);
    try {
      const [nextBatch, nextReview] = await Promise.all([
        getProductionQueue(projectId, batchId),
        getProductionBatchReviewProductivity(projectId, batchId),
      ]);
      if (!productionMonitorMounted.current || productionMonitorBatchRef.current !== batchId) return;
      setProductionMonitorBatch(nextBatch);
      setProductionMonitorReview(nextReview);
    } catch (monitorError: unknown) {
      if (productionMonitorMounted.current && productionMonitorBatchRef.current === batchId) {
        setProductionMonitorError(toUserMessage(monitorError));
      }
    } finally {
      productionMonitorRequest.current = false;
      if (productionMonitorMounted.current) setProductionMonitorLoading(false);
      const pendingBatchId = productionMonitorPendingBatch.current;
      productionMonitorPendingBatch.current = undefined;
      if (pendingBatchId && pendingBatchId !== batchId && productionMonitorBatchRef.current === pendingBatchId) {
        void refreshProductionMonitor(pendingBatchId);
      }
    }
  }, [projectId]);

  useEffect(() => { void reload(); }, [reload]);

  useEffect(() => {
    productionMonitorMounted.current = true;
    return () => { productionMonitorMounted.current = false; };
  }, []);

  useEffect(() => {
    productionMonitorBatchRef.current = selectedProductionBatchId;
    setProductionMonitorBatch(undefined);
    setProductionMonitorReview(undefined);
    setProductionMonitorError(undefined);
    setMonitorPreviewAsset(undefined);
    if (mode !== "production" || !selectedProductionBatchId) return;
    if (typeof document !== "undefined" && document.visibilityState === "hidden") return;
    void refreshProductionMonitor(selectedProductionBatchId);
  }, [mode, refreshProductionMonitor, selectedProductionBatchId]);

  useEffect(() => {
    if (mode !== "production" || !selectedProductionBatchId || isTerminalProductionBatch(productionMonitorBatch)) return;
    const refreshIfVisible = () => {
      if (typeof document !== "undefined" && document.visibilityState === "hidden") return;
      void refreshProductionMonitor(selectedProductionBatchId);
    };
    const onVisibilityChange = () => {
      if (document.visibilityState === "visible") refreshIfVisible();
    };
    const intervalId = window.setInterval(refreshIfVisible, 3000);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      window.clearInterval(intervalId);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [mode, productionMonitorBatch, refreshProductionMonitor, selectedProductionBatchId]);

  useEffect(() => {
    if (mode !== "production") return undefined;
    let active = true;
    let refreshTimer: number | undefined;
    let unlisten: (() => void) | undefined;
    void subscribeTaskUpdates((task) => {
      if (!active || task.projectId !== projectId || !["SUCCEEDED", "FAILED", "CANCELLED"].includes(task.status)) return;
      if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
      // The queue runner persists the terminal batch projection shortly after the Task event.
      refreshTimer = window.setTimeout(() => {
        if (!active) return;
        refreshTimer = undefined;
        if (productionModeTab === "multi-package") {
          void refreshMultiPackageBoard();
        } else {
          void reloadProductionQueues();
        }
        const batchId = productionMonitorBatchRef.current;
        if (batchId) void refreshProductionMonitor(batchId);
      }, 900);
    })
      .then((cleanup) => {
        if (active) unlisten = cleanup;
        else cleanup();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
      unlisten?.();
    };
  }, [mode, productionModeTab, projectId, refreshMultiPackageBoard, refreshProductionMonitor, reloadProductionQueues]);

  useEffect(() => {
    if (mode !== "production") return;
    setProductionQueueExpanded(true);
    void reloadProductionQueues();
  }, [mode, reloadProductionQueues]);

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
    if (entry.kind === "prompt" && isPromptTemplateText(version.text)) {
      setNotice("这是提示词模板，请在下方预览并确认后应用；不会把 {{variable}} 原样保存到镜头。");
      return;
    }
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
    if (!recipe) throw new Error(`当前没有可用的${nextStage === "image" ? "Krea2 图片" : "H3 视频"}配方。`);
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
    setNotice("已打开结构管理；系列 / 集 / 场景的新增、重命名、排序和归档仍由原有管理面板执行。");
  }

  function handleStructureCreate(target: ProjectStructureCreateTarget, context: WorkspaceSelection) {
    if (target === "shot") {
      void addShot();
      return;
    }
    openStructureManagement(context);
  }

  const focusProductionQueueBatch = useCallback((batchId: string) => {
    productionMonitorBatchRef.current = batchId;
    setFocusedProductionBatchId(batchId);
    setProductionQueueExpanded(true);
  }, []);

  const openProductionQueue = useCallback(async (result?: ProductionPackageCreateBatchesResult) => {
    const createdBatchIds = result?.batches.map((batch) => batch.batchId) ?? [];
    const firstBatchId = createdBatchIds[0];
    if (result) setRecentlyCreatedProductionBatchIds(createdBatchIds);
    if (firstBatchId) focusProductionQueueBatch(firstBatchId);
    const snapshot = await reloadProductionQueues(true);
    if (firstBatchId && !snapshot?.queues.some((queue) => queue.id === firstBatchId)) {
      throw new Error("Created production batch is not visible in queue projection");
    }
    setProductionQueueExpanded(true);
    onOpenProductionQueue?.();
  }, [focusProductionQueueBatch, onOpenProductionQueue, reloadProductionQueues]);

  const openProductionMonitorBatch = useCallback((batchId: string) => {
    focusProductionQueueBatch(batchId);
    if (typeof document === "undefined" || document.visibilityState !== "hidden") {
      void refreshProductionMonitor(batchId);
    }
  }, [focusProductionQueueBatch, refreshProductionMonitor]);

  const startProductionBatch = useCallback(async (batchId: string) => {
    focusProductionQueueBatch(batchId);
    await startProductionQueue(projectId, batchId);
    await reloadProductionQueues();
    await reload();
    await refreshProductionMonitor(batchId);
  }, [focusProductionQueueBatch, projectId, refreshProductionMonitor, reload, reloadProductionQueues]);

  const requeueProductionMonitorItem = useCallback(async (itemId: string) => {
    const batchId = selectedProductionBatchId;
    if (!batchId) return;
    setProductionMonitorError(undefined);
    try {
      await requeueProductionQueueItem(projectId, batchId, itemId);
      await reloadProductionQueues();
      await refreshProductionMonitor(batchId);
      setNotice("已重新加入当前批次等待队列；不会自动开始。 ");
    } catch (requeueError: unknown) {
      setProductionMonitorError(toUserMessage(requeueError));
    }
  }, [projectId, refreshProductionMonitor, reloadProductionQueues, selectedProductionBatchId]);

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
      if (productionMonitorBatchRef.current !== batchId || selectedProductionBatchId !== batchId) {
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
  }, [projectId, selectedProductionBatchId]);
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
    const discoveredPackage = multiPackagePackages.find((item) => multiPackageIdentity(item) === packageKey);
    if (!discoveredPackage) return;
    setProductionPackageFolderPath(discoveredPackage.packageRoot);
    setProductionPackageWorkspaceKey((current) => current + 1);
    setProductionModeTab("package");
    setNotice(`已打开「${discoveredPackage.relativePath || discoveredPackage.packageRoot}」单生产包工作区；不会自动创建或开始批次。`);
  }, [multiPackagePackages]);
  const reinspectMultiPackage = useCallback(async (packageKey: string) => {
    if (multiPackageDiscovering || multiPackageCreating || !multiPackageMounted.current) return;
    const discoveredPackage = multiPackagePackages.find((item) => item.packageKey === packageKey);
    if (!discoveredPackage) {
      setError("该生产包尚未完成发现，请重新选择根目录。");
      return;
    }
    setError(undefined);
    try {
      const inspection = await inspectProductionPackage(projectId, discoveredPackage.packageRoot);
      if (!multiPackageMounted.current) return;
      if (inspection.manifestSha256 !== discoveredPackage.manifestSha256) {
        throw new Error("production-package.json 在发现后发生变化，请重新选择根目录。");
      }
      setMultiPackageInspections((current) => ({ ...current, [packageKey]: inspection }));
      setMultiPackageInspectionErrors((current) => {
        const next = { ...current };
        delete next[packageKey];
        return next;
      });
      setMultiPackageCreateMessages((current) => {
        const next = { ...current };
        delete next[packageKey];
        return next;
      });
      await refreshMultiPackageBoard();
    } catch (inspectionError: unknown) {
      if (!multiPackageMounted.current) return;
      const message = toUserMessage(inspectionError);
      setMultiPackageInspectionErrors((current) => ({ ...current, [packageKey]: message }));
      setError("重新检查生产包失败：" + message);
    }
  }, [inspectProductionPackage, multiPackageCreating, multiPackageDiscovering, multiPackagePackages, projectId, refreshMultiPackageBoard]);
  const openMultiPackageBatch = useCallback((_packageKey: string, batchIds: string[]) => {
    const batchId = batchIds.reduce<string | undefined>((selected, candidate) => {
      if (!selected) return candidate;
      return multiPackageBatchOpenPriority(multiPackageBatchDetails[candidate])
        < multiPackageBatchOpenPriority(multiPackageBatchDetails[selected])
        ? candidate
        : selected;
    }, undefined);
    if (!batchId) return;
    focusProductionQueueBatch(batchId);
    void openProductionQueue()
      .then(() => openProductionMonitorBatch(batchId))
      .catch((openError: unknown) => setError(`打开生产批次失败：${toUserMessage(openError)}`));
  }, [focusProductionQueueBatch, multiPackageBatchDetails, openProductionMonitorBatch, openProductionQueue]);
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
              onOpenTask={onOpenTask}
              history={stageLinks}
              onRetry={(link) => retryShot(selectedShot?.id ?? "", link.stage)}
              onDeleteShot={() => void removeShot()}
              onCreateShot={() => void addShot()}
              onCopyPrompt={(prompt) => void navigator.clipboard?.writeText(prompt).then(() => setNotice("提示词已复制。"))}
              workspaceTab={shotWorkspaceTab}
              onWorkspaceTabChange={setShotWorkspaceTab}
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
              onPreviewPrompt={() => setNotice("提示词预览使用当前编辑框内容；保存镜头后才会写入快照。")}
              onApplyPrompt={() => setNotice("当前提示词预览已应用到编辑框；点击保存镜头写入快照。")}
              notice={notice}
              error={error}
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
        onStart={startProductionBatch}
        onPause={async (batchId) => { await pauseProductionQueue(projectId, batchId); await reloadProductionQueues(); await reload(); }}
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

function multiPackageIdentity(discoveredPackage: ProductionPackageDiscoveryPackage): string {
  return discoveredPackage.packageKey;
}

function multiPackageInspectionSafetyError(inspection: ProductionPackageInspectionResult): string | undefined {
  const hasWarning = inspection.status === "WARNING"
    || inspection.warningCount > 0
    || inspection.items.some((item) => item.status === "WARNING")
    || Boolean(inspection.warnings?.length);
  if (hasWarning) return "该生产包包含需要人工确认的警告镜头，请先在单生产包中处理。";

  const hasBlocked = inspection.status === "BLOCKED"
    || inspection.blockedCount > 0
    || inspection.items.some((item) => item.status === "BLOCKED")
    || Boolean(inspection.errors?.length);
  if (hasBlocked) return "该生产包包含阻塞项目，不能批量创建。";

  const hasUnsupportedStatus = inspection.status !== undefined && inspection.status !== "READY";
  const hasUnsupportedItem = inspection.items.some((item) => item.status !== "READY");
  if (hasUnsupportedStatus || hasUnsupportedItem) return "该生产包当前检查状态不是 READY，不能批量创建。";
  return undefined;
}

function buildMultiPackageBoardPackage(input: {
  discoveredPackage: ProductionPackageDiscoveryPackage;
  inspection?: ProductionPackageInspectionResult;
  inspectionError?: string;
  bindings: ProductionPackageBatchBinding[];
  batchDetails: Record<string, ProductionBatchDetail>;
  createMessage?: { status: "CREATE_FAILED" | "NOT_CREATED"; message: string };
}): MultiPackageBoardPackage {
  const { discoveredPackage, inspection, inspectionError, bindings, batchDetails, createMessage } = input;
  const packageBindings = bindings.filter(
    (binding) => binding.packageKey === discoveredPackage.packageKey,
  );
  const packageBatchIds = [...new Set(packageBindings.map((binding) => binding.batchId))];
  const details = packageBatchIds
    .map((batchId) => batchDetails[batchId])
    .filter((detail): detail is ProductionBatchDetail => Boolean(detail));
  const stats = details.reduce((current, detail) => ({
    pending: current.pending + detail.pending,
    running: current.running + detail.running,
    succeeded: current.succeeded + detail.succeeded,
    failed: current.failed + detail.failed,
  }), { pending: 0, running: 0, succeeded: 0, failed: 0 });
  const currentItems = inspection?.items ?? [];
  const currentItemIds = new Set(currentItems.map((item) => item.id));
  const boundItemIds = new Set(
    packageBindings
      .flatMap((binding) => binding.packageItemIds)
      .filter((itemId) => currentItemIds.has(itemId)),
  );
  const itemCount = inspection?.itemCount ?? currentItems.length;
  const remainingItems = currentItems.filter((item) => !boundItemIds.has(item.id));
  const boundItemCount = boundItemIds.size;
  const remainingCount = remainingItems.length;
  const remainingReadyCount = remainingItems.filter((item) => item.status === "READY").length;
  const remainingWarningCount = remainingItems.filter((item) => item.status === "WARNING").length;
  const remainingBlockedCount = remainingItems.filter((item) => item.status === "BLOCKED").length;
  const hasActiveBatch = details.some((detail) => detail.status === "RUNNING" || detail.running > 0);
  const allBatchDetailsAvailable = details.length === packageBatchIds.length;
  const allBatchesTerminal = packageBatchIds.length > 0 && allBatchDetailsAvailable && details.every((detail) => (
    detail.status === "COMPLETED"
      || detail.succeeded + detail.failed + detail.cancelled + detail.skipped >= detail.total
  ));
  const allItemsBound = itemCount > 0 && currentItems.length === itemCount && remainingCount === 0;
  const hasInspectionWarnings = Boolean(
    inspection?.status === "WARNING"
      || (inspection?.warnings?.length ?? 0) > 0
      || (inspection?.warningCount ?? 0) > 0
      || currentItems.some((item) => item.status === "WARNING"),
  );
  const hasInspectionBlocked = Boolean(
    inspection?.status === "BLOCKED"
      || (inspection?.errors?.length ?? 0) > 0
      || (inspection?.blockedCount ?? 0) > 0
      || currentItems.some((item) => item.status === "BLOCKED"),
  );
  let status: MultiPackageBoardPackage["status"];
  if (createMessage) status = createMessage.status;
  else if (inspectionError || !inspection) status = "BLOCKED";
  else if (!packageBindings.length) status = hasInspectionBlocked
    ? "BLOCKED"
    : hasInspectionWarnings ? "WARNING" : "READY";
  else if (hasActiveBatch) status = "RUNNING";
  else if (allItemsBound && allBatchesTerminal && stats.failed > 0) status = "COMPLETED_WITH_FAILURE";
  else if (allItemsBound && allBatchesTerminal) status = "COMPLETED";
  else if (allItemsBound) status = "CREATED";
  else status = "PARTIAL";

  const canCreate = !createMessage
    ? (status === "READY" || status === "PARTIAL")
      && remainingCount > 0
      && remainingReadyCount === remainingCount
      && remainingWarningCount === 0
      && remainingBlockedCount === 0
    : createMessage.status === "NOT_CREATED"
      && remainingCount > 0
      && remainingReadyCount === remainingCount
      && remainingWarningCount === 0
      && remainingBlockedCount === 0;
  const inspectionWarningCount = inspection?.warningCount ?? 0;
  const inspectionBlockedCount = inspection?.blockedCount ?? 0;

  const issueSummary = createMessage?.message
    ?? inspectionError
    ?? (inspection?.errors?.length ? inspection.errors.map(packageDiagnosticText).join("；") : undefined)
    ?? (inspection && (hasInspectionWarnings || inspectionWarningCount > 0 || inspectionBlockedCount > 0)
      ? `READY ${inspection.readyCount} · WARNING ${inspectionWarningCount} · BLOCKED ${inspectionBlockedCount}`
      : undefined);
  const firstError = createMessage?.message
    ?? details.flatMap((detail) => detail.items)
      .map((item) => item.errorMessage || item.errorCode)
      .find((value): value is string => Boolean(value));
  return {
    packageKey: multiPackageIdentity(discoveredPackage),
    packageRoot: discoveredPackage.packageRoot,
    relativePath: discoveredPackage.relativePath,
    packageName: inspection?.packageName ?? displayMultiPackageName(discoveredPackage),
    itemCount,
    status,
    readyCount: inspection?.readyCount ?? 0,
    warningCount: inspection?.warningCount ?? 0,
    blockedCount: inspection?.blockedCount ?? 0,
    boundItemCount,
    remainingCount,
    remainingReadyCount,
    remainingWarningCount,
    remainingBlockedCount,
    canCreate,
    batchIds: packageBatchIds,
    pending: stats.pending + remainingCount,
    running: stats.running,
    succeeded: stats.succeeded,
    failed: stats.failed,
    firstError,
    issueSummary,
  };
}

function packageDiagnosticText(issue: unknown): string {
  if (typeof issue === "string") return issue;
  if (issue && typeof issue === "object") {
    const value = issue as { code?: unknown; message?: unknown; detail?: unknown };
    return [value.code, value.message, value.detail].filter((item): item is string => typeof item === "string" && item.length > 0).join("：");
  }
  return "未知问题";
}

function displayMultiPackageName(discoveredPackage: ProductionPackageDiscoveryPackage): string {
  const source = discoveredPackage.relativePath || discoveredPackage.packageRoot;
  return source.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || source;
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
