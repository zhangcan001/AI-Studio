import { useEffect, useMemo, useRef, useState } from "react";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import {
  createProductionPackageBatches,
  inspectProductionPackage,
  type ProductionPackageCreateBatchesResult,
  type ProductionPackageInspectionResult,
} from "../../services/tauriClient";
import { toUserMessage } from "../../i18n/errorMessages";
import type {
  ProductionPackageInspectionItem,
  ProductionPackageIssue,
} from "../../types/productionPackage";
import { ProductionPackagePreview } from "./ProductionPackagePreview";
import "./ProductionPackageWorkspace.css";

export const PRODUCTION_PACKAGE_WORKSPACE_PAGE_SIZE = 50;

export type ProductionPackageWorkspaceState =
  | "EMPTY"
  | "DRAG_OVER"
  | "INSPECTING"
  | "READY"
  | "PARTIAL"
  | "BLOCKED"
  | "CREATING_BATCHES"
  | "CREATED"
  | "ERROR";

export type ProductionPackageFolderPickerResult =
  | string
  | { path?: string | null; folderPath?: string | null }
  | null
  | undefined;

export type ProductionPackageFolderPicker = () =>
  | ProductionPackageFolderPickerResult
  | Promise<ProductionPackageFolderPickerResult>;

export interface ProductionPackageWorkspaceProps {
  projectId: string;
  /** A parent-controlled package root. A new non-empty value is inspected automatically. */
  folderPath?: string | null;
  /** Called when the workspace receives a path from its picker. */
  onFolderPathChange?: (folderPath: string) => void;
  /** Optional parent notification after a folder has been selected. */
  onFolderSelected?: (folderPath: string) => void | Promise<void>;
  /** Alias for hosts that use an explicit selected-path callback. */
  onFolderPathSelected?: (folderPath: string) => void | Promise<void>;
  /** Any one of these picker callbacks can return a path; the result is inspected immediately. */
  onChooseFolder?: ProductionPackageFolderPicker;
  onPickFolder?: ProductionPackageFolderPicker;
  onSelectFolder?: ProductionPackageFolderPicker;
  onOpenFolderPicker?: ProductionPackageFolderPicker;
  folderPicker?: ProductionPackageFolderPicker;
  /** Legacy queue callback. When provided, it is invoked after a successful create. */
  onOpenQueue?: () => void | Promise<void>;
  /** Optional richer queue callback that receives the created batch mapping. */
  onOpenProductionQueue?: (result: ProductionPackageCreateBatchesResult) => void | Promise<void>;
  defaultFolderPath?: string;
}

type WorkspaceInspection = ProductionPackageInspectionResult & {
  packageType?: string;
  status?: string;
  warnings?: ProductionPackageIssue[];
  errors?: ProductionPackageIssue[];
};

interface WorkspaceError {
  message: string;
  code?: string;
  technicalMessage?: string;
  details?: ProductionPackageErrorDetails;
  requiresReinspect: boolean;
}

interface ProductionPackageErrorDetails {
  packageErrorCode?: string;
  technicalMessage?: string;
  mode?: string;
  workflowVersionId?: string;
  recipeId?: string;
  itemId?: string;
  requiresReinspect?: boolean;
}

const STALE_PACKAGE_ERROR_CODES = new Set([
  "PACKAGE_SESSION_EXPIRED",
  "PACKAGE_SESSION_NOT_FOUND",
  "PACKAGE_MEDIA_CHANGED",
  "PACKAGE_PROMPT_CHANGED",
  "PACKAGE_MODE_CHANGED",
  "PACKAGE_ITEM_NOT_FOUND",
  "PACKAGE_ITEM_BLOCKED",
  "PACKAGE_PROJECT_WORKFLOW_CHANGED",
]);

export function ProductionPackageWorkspace({
  projectId,
  folderPath,
  onFolderPathChange,
  onFolderSelected,
  onFolderPathSelected,
  onChooseFolder,
  onPickFolder,
  onSelectFolder,
  onOpenFolderPicker,
  folderPicker,
  onOpenQueue,
  onOpenProductionQueue,
  defaultFolderPath = "",
}: ProductionPackageWorkspaceProps) {
  const isControlledFolderPath = folderPath !== undefined;
  const [localFolderPath, setLocalFolderPath] = useState(defaultFolderPath);
  const currentFolderPath = isControlledFolderPath ? normalizePath(folderPath) : localFolderPath;
  const [inspection, setInspection] = useState<WorkspaceInspection>();
  const [selectedItemIds, setSelectedItemIds] = useState<Set<string>>(() => new Set());
  const [isInspecting, setIsInspecting] = useState(false);
  const [isPickingFolder, setIsPickingFolder] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [isOpeningQueue, setIsOpeningQueue] = useState(false);
  const [createdResult, setCreatedResult] = useState<ProductionPackageCreateBatchesResult>();
  const [queueOpenFailed, setQueueOpenFailed] = useState(false);
  const [isPreviewExpanded, setIsPreviewExpanded] = useState(true);
  const [isDragOver, setIsDragOver] = useState(false);
  const [dropSupport, setDropSupport] = useState<"unknown" | "available" | "unavailable">("unknown");
  const [error, setError] = useState<WorkspaceError>();
  const [notice, setNotice] = useState<string>();
  const requestIdRef = useRef(0);
  const observedFolderKeyRef = useRef<string | undefined>(undefined);
  const preferredSelectionIdsRef = useRef<Set<string> | undefined>(undefined);
  const dropHandlerRef = useRef<(paths: readonly string[]) => void>(() => undefined);

  const pickFolder = onChooseFolder ?? onPickFolder ?? onSelectFolder ?? onOpenFolderPicker ?? folderPicker;
  const busy = isInspecting || isPickingFolder || isCreating || isOpeningQueue;

  function resetWorkspace() {
    requestIdRef.current += 1;
    setInspection(undefined);
    setSelectedItemIds(new Set());
    setCreatedResult(undefined);
    setQueueOpenFailed(false);
    setIsPreviewExpanded(true);
    setIsDragOver(false);
    preferredSelectionIdsRef.current = undefined;
    setIsInspecting(false);
    setError(undefined);
    setNotice(undefined);
  }

  async function inspectFolder(candidatePath: string) {
    const packageRoot = normalizePath(candidatePath);
    if (!packageRoot) {
      resetWorkspace();
      return;
    }

    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    const preferredSelectionIds = preferredSelectionIdsRef.current;
    preferredSelectionIdsRef.current = undefined;
    setIsInspecting(true);
    setInspection(undefined);
    setSelectedItemIds(new Set());
    setCreatedResult(undefined);
    setError(undefined);
    setNotice(undefined);

    try {
      const result = await inspectProductionPackage(projectId, packageRoot);
      if (requestId !== requestIdRef.current) return;
      const nextInspection = result as WorkspaceInspection;
      setInspection(nextInspection);
      setSelectedItemIds(initialSelection(nextInspection.items, preferredSelectionIds));
      setIsPreviewExpanded(nextInspection.items.length <= 10);
      setQueueOpenFailed(false);
      setNotice(`已检查「${displayFolderName(packageRoot)}」，请确认项目后创建批次。`);
    } catch (inspectionError: unknown) {
      if (requestId !== requestIdRef.current) return;
      setInspection(undefined);
      setSelectedItemIds(new Set());
      setCreatedResult(undefined);
      setError(toWorkspaceError(inspectionError, "inspect"));
    } finally {
      if (requestId === requestIdRef.current) setIsInspecting(false);
    }
  }

  useEffect(() => {
    if (!isControlledFolderPath) return;
    const normalized = normalizePath(folderPath);
    const key = `${projectId}\u0000${normalized}`;
    if (key === observedFolderKeyRef.current) return;
    observedFolderKeyRef.current = key;
    if (!normalized) resetWorkspace();
    else void inspectFolder(normalized);
  }, [folderPath, isControlledFolderPath, projectId]);

  function adoptFolderPath(nextPath: string) {
    const normalized = normalizePath(nextPath);
    if (isControlledFolderPath) {
      observedFolderKeyRef.current = `${projectId}\u0000${normalized}`;
      onFolderPathChange?.(normalized);
    } else {
      setLocalFolderPath(normalized);
    }
    void Promise.resolve(onFolderSelected?.(normalized)).catch(() => undefined);
    void Promise.resolve(onFolderPathSelected?.(normalized)).catch(() => undefined);
    if (!normalized) resetWorkspace();
    else void inspectFolder(normalized);
  }

  function handleDroppedPaths(paths: readonly string[]) {
    setIsDragOver(false);
    if (busy) return;
    if (paths.length !== 1) {
      setError({
        message: "一次只能拖入一个 Production Package 文件夹。",
        code: "PACKAGE_DROP_INVALID",
        requiresReinspect: false,
      });
      setNotice(undefined);
      return;
    }

    const droppedPath = normalizePath(paths[0]);
    if (!droppedPath || isLikelyDroppedFile(droppedPath)) {
      setError({
        message: "请拖入包含 production-package.json 的整个 Production Package 文件夹。",
        code: "PACKAGE_DROP_INVALID",
        requiresReinspect: false,
      });
      setNotice(undefined);
      return;
    }
    adoptFolderPath(droppedPath);
  }

  dropHandlerRef.current = handleDroppedPaths;

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    async function registerDropListener() {
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        if (disposed) return;
        const listener = await getCurrentWebview().onDragDropEvent((event) => {
          const payload = event.payload as DragDropEvent;
          if (payload.type === "enter" || payload.type === "over") {
            if (!busy) setIsDragOver(true);
            return;
          }
          setIsDragOver(false);
          if (payload.type === "drop") dropHandlerRef.current(payload.paths);
        });
        if (disposed) listener();
        else {
          unlisten = listener;
          setDropSupport("available");
        }
      } catch {
        if (!disposed) setDropSupport("unavailable");
      }
    }

    void registerDropListener();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function chooseFolder() {
    if (!pickFolder || busy) return;
    setIsPickingFolder(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const picked = await pickFolder();
      const pickedPath = typeof picked === "string"
        ? picked
        : picked?.path ?? picked?.folderPath;
      if (pickedPath) adoptFolderPath(pickedPath);
    } catch (pickError: unknown) {
      setError(toWorkspaceError(pickError, "inspect"));
    } finally {
      setIsPickingFolder(false);
    }
  }

  function manuallyInspect() {
    if (busy || !currentFolderPath) return;
    void inspectFolder(currentFolderPath);
  }

  function reinspectRemaining() {
    if (busy || !currentFolderPath || !createdResult?.remainingItemIds.length) return;
    preferredSelectionIdsRef.current = new Set(createdResult.remainingItemIds);
    void inspectFolder(currentFolderPath);
  }

  function chooseNextPackage() {
    if (busy) return;
    if (isControlledFolderPath) {
      observedFolderKeyRef.current = `${projectId}\u0000`;
      onFolderPathChange?.("");
    } else {
      setLocalFolderPath("");
    }
    resetWorkspace();
  }

  const items = inspection?.items ?? [];
  const counts = useMemo(() => summarizeItems(inspection), [inspection]);
  const derivedInspectionState = deriveInspectionState(inspection);
  const selectedItems = useMemo(
    () => items.filter((item) => isSelectableItem(item) && selectedItemIds.has(item.id)),
    [items, selectedItemIds],
  );
  const selectedReadyCount = selectedItems.filter((item) => item.status === "READY").length;
  const selectedWarningCount = selectedItems.filter((item) => item.status === "WARNING").length;
  const selectionIdsForCreate = useMemo(
    () => items
      .filter((item) => selectedItemIds.has(item.id))
      .map((item) => item.id),
    [items, selectedItemIds],
  );
  const requiresReinspect = Boolean(error?.requiresReinspect);
  const canCreate = Boolean(
    inspection
      && inspection.inspectionId
      && selectionIdsForCreate.length > 0
      && !busy
      && !createdResult
      && !requiresReinspect
      && (derivedInspectionState === "READY" || derivedInspectionState === "PARTIAL"),
  );
  const createdCount = createdResult?.createdCount ?? createdResult?.itemCount ?? 0;
  const requestedCount = createdResult?.requestedCount ?? createdCount + (createdResult?.remainingCount ?? 0);
  const remainingCount = createdResult?.remainingCount ?? createdResult?.remainingItemIds?.length ?? 0;
  const isPartialCreate = createdResult?.status === "PARTIAL" || remainingCount > 0;
  const workspaceState: ProductionPackageWorkspaceState = isDragOver && !busy
    ? "DRAG_OVER"
    : isInspecting || isPickingFolder
      ? "INSPECTING"
      : isCreating
        ? "CREATING_BATCHES"
        : error
          ? "ERROR"
          : createdResult
            ? isPartialCreate ? "PARTIAL" : "CREATED"
            : derivedInspectionState;
  const hasQueueCallback = Boolean(onOpenQueue || onOpenProductionQueue);
  const folderStatusMessage = folderStatusLabel({
    hasPath: Boolean(currentFolderPath),
    isInspecting: isInspecting || isPickingFolder,
    hasCreatedResult: Boolean(createdResult),
    hasError: Boolean(error),
    isBlocked: workspaceState === "BLOCKED",
    hasInspection: Boolean(inspection),
  });

  function handlePreviewSelectionChange(nextSelection: Set<string>) {
    if (busy) return;
    const selectableIds = new Set(items.filter(isSelectableItem).map((item) => item.id));
    setSelectedItemIds(new Set([...nextSelection].filter((itemId) => selectableIds.has(itemId))));
    setError(undefined);
    setNotice(undefined);
  }

  async function createBatches() {
    if (!inspection || !canCreate) return;
    const ids = [...selectionIdsForCreate];
    setIsCreating(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const result = await createProductionPackageBatches(inspection.inspectionId, ids);
      setCreatedResult(result);
      setQueueOpenFailed(false);
      const createdCount = result.createdCount ?? result.itemCount;
      const remainingCount = result.remainingCount ?? result.remainingItemIds?.length ?? 0;
      setNotice(hasQueueCallback
        ? "批次已创建，正在打开生产队列…"
        : result.status === "PARTIAL"
          ? `批次创建部分完成：已加入 ${createdCount} 个项目，尚未加入 ${remainingCount} 个项目；不会自动启动队列。`
          : `已创建 ${result.batchCount} 个生产批次，共 ${createdCount} 个项目；不会自动启动队列。`);
      if (hasQueueCallback) await openQueue(result);
    } catch (createError: unknown) {
      setError(toWorkspaceError(createError, "create"));
    } finally {
      setIsCreating(false);
    }
  }

  async function openQueue(resultOverride?: ProductionPackageCreateBatchesResult) {
    const resultToOpen = resultOverride ?? createdResult;
    if (!resultToOpen || !hasQueueCallback || isOpeningQueue) return;
    setIsOpeningQueue(true);
    setError(undefined);
    try {
      if (onOpenProductionQueue) await onOpenProductionQueue(resultToOpen);
      else await onOpenQueue?.();
      setQueueOpenFailed(false);
      setNotice("已打开生产队列；请点击“开始”后才会提交生成任务。 ");
    } catch (openError: unknown) {
      const openFailure = toWorkspaceError(openError, "open");
      setQueueOpenFailed(true);
      setError({
        ...openFailure,
        message: "生产批次已创建，但生产队列暂时无法打开。",
        requiresReinspect: false,
      });
    } finally {
      setIsOpeningQueue(false);
    }
  }

  const statusMessage = createdResult
    ? queueOpenFailed
      ? "生产批次已创建，但生产队列暂时无法打开。"
      : isPartialCreate
        ? `批次创建部分完成：已加入 ${createdCount} 个项目，尚有 ${remainingCount} 个项目待重新检查。`
        : hasQueueCallback
          ? "生产批次已创建并已打开生产队列；不会自动开始生成。"
          : "生产批次已创建；不会自动开始生成。"
    : statusMessageForState(workspaceState, counts, selectedItems.length);

  return (
    <section
      className="production-package-workspace"
      data-state={workspaceState}
      data-selected-count={selectedItems.length}
      aria-label="Production Package 工作区"
      aria-busy={busy}
    >
      <div className="production-package-workspace-heading">
        <div>
          <span className="section-label">External Production Package V1</span>
          <h2>{inspection?.packageName || "批量视频生产"}</h2>
          <p className="section-description">选择或拖入外部智能体准备好的 Production Package 文件夹；检查后创建并打开队列，开始生产仍由你明确点击。</p>
        </div>
        <span className={`production-package-workspace-state production-package-workspace-state-${workspaceState.toLowerCase()}`}>
          {workspaceStateLabel(workspaceState)}
        </span>
      </div>

      <div className="production-package-workspace-folder" aria-label="生产包文件夹入口" data-drop-support={dropSupport}>
        <div
          className={`production-package-workspace-drop-zone${isDragOver ? " production-package-workspace-drop-zone-active" : ""}`}
          aria-label="Production Package 文件夹拖放区域"
          data-drop-state={isDragOver ? "DRAG_OVER" : "IDLE"}
        >
          <strong>{isDragOver ? "松开以检查 Production Package 文件夹" : "将 Production Package 文件夹拖到这里"}</strong>
          <span>或使用下方的文件夹选择器</span>
          <small>目录中必须包含 <code>production-package.json</code></small>
          {dropSupport === "unavailable" && <small>桌面拖放 API 不可用，请使用文件夹选择器。</small>}
        </div>
        <label htmlFor="production-package-folder-path">Production Package 文件夹路径</label>
        <div className="production-package-workspace-folder-row">
          <input
            id="production-package-folder-path"
            type="text"
            value={currentFolderPath}
            placeholder="尚未选择生产包文件夹"
            readOnly
            aria-readonly="true"
            title={currentFolderPath || undefined}
            disabled={isCreating || isPickingFolder}
            aria-describedby="production-package-folder-help"
          />
          {pickFolder && (
            <button type="button" className="quiet-button" onClick={() => void chooseFolder()} disabled={busy}>
              {isPickingFolder ? "正在选择…" : "选择生产包文件夹"}
            </button>
          )}
          <button type="button" onClick={manuallyInspect} disabled={busy || !currentFolderPath}>
            {isInspecting ? "检查中…" : inspection || error ? "重新检查" : "检查文件夹"}
          </button>
        </div>
        <small className="production-package-workspace-folder-status" role="status" aria-live="polite">{folderStatusMessage}</small>
        <small id="production-package-folder-help">选择结果会自动检查；检查只读，创建批次前后端仍会重新校验路径、媒体和提示词。</small>
        {currentFolderPath && (
          <details className="production-package-workspace-folder-details">
            <summary>已选文件夹详情</summary>
            <dl>
              <div><dt>文件夹名称</dt><dd>{displayFolderName(currentFolderPath)}</dd></div>
              <div><dt>完整路径</dt><dd><code>{currentFolderPath}</code></dd></div>
            </dl>
          </details>
        )}
      </div>

      <details className="production-package-workspace-spec">
        <summary>Production Package V1 规范 / 生产包格式说明</summary>
        <div>
          <p>文件夹根目录必须包含 <code>production-package.json</code>，媒体路径使用相对于该根目录的路径。</p>
          <ul>
            <li>schemaVersion 必须为 1，packageType 必须为 AI_STUDIO_VIDEO_PRODUCTION。</li>
            <li>每个项目需要唯一 ID、名称和非空 videoPrompt；完整提示词不会在预览中展开。</li>
            <li>项目最多 500 个；READY 默认选中，WARNING 需要手动确认，BLOCKED 不可选。</li>
            <li>创建并打开只会加入现有生产队列，不会自动启动生成；开始按钮仍是唯一生产闸门。</li>
          </ul>
        </div>
      </details>

      <p className="production-package-workspace-state-message" role="status" aria-live="polite">
        {statusMessage}
      </p>

      {error && (
        <div className="production-package-workspace-error" role="alert" aria-live="assertive">
          <strong>{error.code ? `操作失败 · ${error.code}` : "操作失败"}</strong>
          <p>{error.message}</p>
          {error.details && (
            <dl aria-label="生产包错误详情">
              {error.details.packageErrorCode && <div><dt>错误类型</dt><dd>{error.details.packageErrorCode}</dd></div>}
              {error.details.mode && <div><dt>模式</dt><dd>{error.details.mode}</dd></div>}
              {error.details.workflowVersionId && <div><dt>WorkflowVersion</dt><dd>{error.details.workflowVersionId}</dd></div>}
              {error.details.recipeId && <div><dt>Recipe</dt><dd>{error.details.recipeId}</dd></div>}
              {error.details.itemId && <div><dt>项目</dt><dd>{error.details.itemId}</dd></div>}
            </dl>
          )}
          {error.technicalMessage && (
            <details open>
              <summary>技术详情</summary>
              <p>原因：<code>{error.technicalMessage}</code></p>
            </details>
          )}
          {queueOpenFailed
            ? <small>批次已经创建，可重新打开生产队列；不会重复创建批次。</small>
            : currentFolderPath && <small>请点击“重新检查”获取最新检查结果后再继续。</small>}
        </div>
      )}

      {notice && !error && <p className="production-package-workspace-notice" role="status" aria-live="polite">{notice}</p>}

      {inspection && (
        <>
          <div className="production-package-workspace-summary" aria-label="生产包统计">
            <SummaryMetric label="项目数" value={counts.total} />
            <SummaryMetric label="READY" value={counts.ready} tone="ready" />
            <SummaryMetric label="WARNING" value={counts.warning} tone="warning" />
            <SummaryMetric label="BLOCKED" value={counts.blocked} tone="blocked" />
          </div>

          {(inspection.warnings?.length || inspection.errors?.length) ? (
            <div className="production-package-workspace-diagnostics" role={inspection.errors?.length ? "alert" : "status"}>
              <strong>{inspection.errors?.length ? "检查需要处理" : "检查提示"}</strong>
              <ul aria-label="生产包检查诊断">
                {[...(inspection.errors ?? []), ...(inspection.warnings ?? [])].map((issue, index) => (
                  <li key={`${diagnosticText(issue)}-${index}`}>{diagnosticText(issue)}</li>
                ))}
              </ul>
            </div>
          ) : null}

          <details className="production-package-workspace-resolution-details">
            <summary>查看每个项目的生产工作流配对（{items.length}）</summary>
            <ul aria-label="生产包项目工作流配对">
              {items.map((item) => {
                return (
                  <li key={`resolution-${item.id}`}>
                    <strong>{item.name || item.id}</strong>
                    <span>模式：{item.mode || "—"}</span>
                    <span>WorkflowVersion：{item.resolvedWorkflowVersionId || "未解析"}</span>
                    <span>Recipe：{item.resolvedRecipeId || "未解析"}</span>
                    <span>来源：{item.workflowResolutionSource || "—"}</span>
                    <span>兼容性：{item.recipeCompatibility || item.status}</span>
                  </li>
                );
              })}
            </ul>
          </details>

          <p className="production-package-workspace-selection-summary" role="status" aria-live="polite">
            已选择 {selectedItems.length} 项（READY {selectedReadyCount}，WARNING {selectedWarningCount}）；WARNING 需手动选择，BLOCKED 不可选。
          </p>

          <details
            className="production-package-workspace-preview-details"
            open={isPreviewExpanded}
            onToggle={(event) => setIsPreviewExpanded(event.currentTarget.open)}
          >
            <summary>{isPreviewExpanded ? "收起镜头明细" : `查看 ${items.length} 个镜头明细`}</summary>
            <fieldset disabled={busy} className="production-package-workspace-preview-fieldset">
              <ProductionPackagePreview
                inspection={inspection}
                selectedItemIds={selectedItemIds}
                onSelectionChange={handlePreviewSelectionChange}
                pageSize={50}
              />
            </fieldset>
          </details>
        </>
      )}

      {createdResult && (
        <section
          className={`production-package-workspace-created${isPartialCreate ? " production-package-workspace-created-partial" : ""}`}
          aria-label="生产包创建结果"
        >
          <div className="production-package-workspace-created-heading">
            <div>
              <span className="section-label">CREATE RESULT</span>
              <h3>{isPartialCreate ? "批次创建部分完成" : `已创建 ${createdResult.batchCount} 个生产批次`}</h3>
            </div>
            <span className="production-package-workspace-created-count">{isPartialCreate ? `${createdCount} 个项目已加入` : `${createdCount} 个项目`}</span>
          </div>
          <div className="production-package-workspace-created-summary" role="status">
            <strong>已加入生产：{createdCount}</strong>
            <span>请求项目：{requestedCount}</span>
            <span>尚未加入：{remainingCount}</span>
            <span>状态：{isPartialCreate ? "部分完成" : "完成"}</span>
            <span>自动启动：{createdResult.autoStarted ? "是" : "否"}</span>
          </div>
          <p>{queueOpenFailed
            ? "生产批次已创建，但生产队列暂时无法打开。"
            : hasQueueCallback
              ? "生产队列已打开；不会自动开始生成，请在队列中点击“开始”。"
              : "批次已创建；父层未接入队列打开回调，不会自动启动生成。"}</p>
          <details className="production-package-workspace-batch-details">
            <summary>查看批次详细信息（{createdResult.batchCount} 个）</summary>
            <ul className="production-package-workspace-batch-list" aria-label="已创建生产批次">
              {createdResult.batches.map((batch, index) => (
                <li key={batch.batchId}>
                  <strong>{batch.batchName || `批次 ${index + 1}`}</strong>
                  <span>{batch.itemCount} 个项目 · {batch.batchId}</span>
                </li>
              ))}
            </ul>
          </details>
          {isPartialCreate && remainingCount > 0 && (
            <div className="production-package-workspace-remaining">
              <p>保留 {remainingCount} 个未加入项目的外部 ID；重新检查后只会按最新检查结果选择仍可生产的剩余项目。</p>
              <button type="button" onClick={reinspectRemaining} disabled={busy || !currentFolderPath}>
                重新检查剩余项目
              </button>
            </div>
          )}
          <button type="button" onClick={() => void openQueue()} disabled={!hasQueueCallback || isOpeningQueue}>
            {isOpeningQueue ? "正在打开…" : queueOpenFailed ? "重新打开生产队列" : isPartialCreate ? "打开已创建队列" : "打开生产队列"}
          </button>
          {!hasQueueCallback && <small className="production-package-workspace-queue-note">父层尚未接入打开生产队列回调。</small>}
          <button type="button" className="quiet-button" onClick={chooseNextPackage} disabled={busy}>选择下一个生产包</button>
        </section>
      )}

      <div className="production-package-workspace-footer">
        <button
          type="button"
          className="production-package-workspace-create-button"
          onClick={() => void createBatches()}
          disabled={!canCreate}
        >
          {isCreating ? "正在创建批次…" : createdResult ? "批次已创建" : `创建并打开生产队列${selectionIdsForCreate.length ? `（${selectionIdsForCreate.length} 项）` : ""}`}
        </button>
        {workspaceState === "BLOCKED" && <small>没有可创建的 READY/WARNING 项目，请修复生产包后重新检查。</small>}
        {workspaceState === "ERROR" && !requiresReinspect && !queueOpenFailed && <small>可先重新检查，也可修复错误后再次创建批次。</small>}
      </div>
    </section>
  );
}

function SummaryMetric({ label, value, tone }: { label: string; value: number; tone?: "ready" | "warning" | "blocked" }) {
  return (
    <div className={tone ? `production-package-workspace-metric production-package-workspace-metric-${tone}` : "production-package-workspace-metric"}>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function summarizeItems(inspection: WorkspaceInspection | undefined): { total: number; ready: number; warning: number; blocked: number } {
  if (!inspection) return { total: 0, ready: 0, warning: 0, blocked: 0 };
  const items = inspection.items ?? [];
  return {
    total: inspection.itemCount ?? items.length,
    ready: inspection.readyCount ?? items.filter((item) => item.status === "READY").length,
    warning: inspection.warningCount ?? items.filter((item) => item.status === "WARNING").length,
    blocked: inspection.blockedCount ?? items.filter((item) => item.status === "BLOCKED").length,
  };
}

function initialSelection(
  items: ProductionPackageInspectionItem[],
  preferredIds?: Set<string>,
): Set<string> {
  return new Set(
    items
      .filter((item) => item.status === "READY")
      .filter((item) => !preferredIds || preferredIds.has(item.id))
      .map((item) => item.id),
  );
}

function deriveInspectionState(inspection: WorkspaceInspection | undefined): ProductionPackageWorkspaceState {
  if (!inspection) return "EMPTY";
  if (inspection.errors?.length) return "BLOCKED";
  const items = inspection.items ?? [];
  const selectableCount = items.filter(isSelectableItem).length;
  if (selectableCount === 0 || items.length === 0) return "BLOCKED";
  if (items.every((item) => item.status === "READY")) return "READY";
  return "PARTIAL";
}

function isSelectableItem(item: ProductionPackageInspectionItem): boolean {
  return item.status === "READY" || item.status === "WARNING";
}

function workspaceStateLabel(state: ProductionPackageWorkspaceState): string {
  switch (state) {
    case "EMPTY": return "等待生产包";
    case "DRAG_OVER": return "拖放到这里";
    case "INSPECTING": return "正在检查生产包…";
    case "READY": return "检查完成";
    case "PARTIAL": return "部分项目需要确认";
    case "BLOCKED": return "生产包存在问题";
    case "CREATING_BATCHES": return "正在创建批次";
    case "CREATED": return "批次已创建";
    case "ERROR": return "需要处理错误";
  }
}

function statusMessageForState(
  state: ProductionPackageWorkspaceState,
  counts: { total: number; ready: number; warning: number; blocked: number },
  selectedCount: number,
): string {
  switch (state) {
    case "EMPTY": return "尚未选择生产包。请拖入或选择包含 production-package.json 的目录。";
    case "DRAG_OVER": return "已识别拖放操作；松开后将自动检查生产包。";
    case "INSPECTING": return "正在检查 Production Package，请稍候。";
    case "READY": return `检查完成：${counts.total} 个项目全部 READY，可创建批次。`;
    case "PARTIAL": return `检查完成：${counts.ready} 个 READY、${counts.warning} 个 WARNING、${counts.blocked} 个 BLOCKED；当前已选择 ${selectedCount} 项。`;
    case "BLOCKED": return "检查完成，但没有可创建的 READY 或 WARNING 项目。请修复生产包后重新检查。";
    case "CREATING_BATCHES": return `正在创建批次，${selectedCount} 个已选择项目处理中；操作完成前控件已禁用。`;
    case "CREATED": return "生产批次已创建；队列不会自动开始生成。";
    case "ERROR": return "操作遇到错误；请根据提示重新检查后继续。";
  }
}

function diagnosticText(issue: ProductionPackageIssue): string {
  if (typeof issue === "string") return issue;
  return [issue.code, issue.message, issue.detail].filter(Boolean).join("：") || "未知问题";
}

function normalizePath(path: string | null | undefined): string {
  return path?.trim() ?? "";
}

function displayFolderName(path: string): string {
  const normalized = path.replace(/[\\/]+$/, "");
  return normalized.split(/[\\/]/).pop() || normalized;
}

function isLikelyDroppedFile(path: string): boolean {
  if (/^[a-z][a-z\d+.-]*:\/\//i.test(path)) return true;
  const basename = path.replace(/[\\/]+$/, "").split(/[\\/]/).pop()?.trim().toLowerCase() ?? "";
  return basename === "production-package.json" || /\.[^./\\]+$/.test(basename);
}

function folderStatusLabel(input: {
  hasPath: boolean;
  isInspecting: boolean;
  hasCreatedResult: boolean;
  hasError: boolean;
  isBlocked: boolean;
  hasInspection: boolean;
}): string {
  if (!input.hasPath) return "尚未选择 Production Package 文件夹";
  if (input.isInspecting) return "已选择 · 正在检查";
  if (input.hasCreatedResult) return "已选择 · 已创建生产批次";
  if (input.hasError || input.isBlocked) return "已选择 · 检查发现问题";
  if (input.hasInspection) return "已选择 · 检查完成";
  return "已选择";
}

function statusErrorCode(error: unknown): string | undefined {
  const details = packageErrorDetails(error);
  if (details?.packageErrorCode) return details.packageErrorCode;
  if (error && typeof error === "object" && "code" in error) {
    const code = (error as { code?: unknown }).code;
    if (typeof code === "string" && code) return code;
  }
  const raw = rawErrorText(error);
  return raw.match(/[A-Z][A-Z0-9_]{2,}/)?.[0];
}

function packageErrorDetails(error: unknown): ProductionPackageErrorDetails | undefined {
  if (!error || typeof error !== "object" || !("details" in error)) return undefined;
  const details = (error as { details?: unknown }).details;
  if (!details || typeof details !== "object" || Array.isArray(details)) return undefined;
  const record = details as Record<string, unknown>;
  const stringValue = (value: unknown): string | undefined =>
    typeof value === "string" && value.trim() ? value : undefined;
  return {
    packageErrorCode: stringValue(record.packageErrorCode),
    technicalMessage: stringValue(record.technicalMessage),
    mode: stringValue(record.mode),
    workflowVersionId: stringValue(record.workflowVersionId),
    recipeId: stringValue(record.recipeId),
    itemId: stringValue(record.itemId),
    requiresReinspect: record.requiresReinspect === true,
  };
}

function technicalErrorMessage(error: unknown): string | undefined {
  return packageErrorDetails(error)?.technicalMessage || rawErrorText(error) || undefined;
}

function rawErrorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") return message;
  }
  try {
    return JSON.stringify(error) || String(error);
  } catch {
    return String(error);
  }
}

function toWorkspaceError(error: unknown, operation: "inspect" | "create" | "open"): WorkspaceError {
  const code = statusErrorCode(error);
  const details = packageErrorDetails(error);
  const technicalMessage = technicalErrorMessage(error);
  let message: string;
  switch (code) {
    case "PACKAGE_RECIPE_INCOMPATIBLE":
      message = "工作流不兼容当前生产模式，请调整项目工作流后重新检查。";
      break;
    case "PROJECT_WORKFLOW_UNAVAILABLE_FOR_PACKAGE_MODE":
      message = "当前项目没有可用于该生产模式的工作流，请先配置项目工作流。";
      break;
    case "PACKAGE_H3_IMPORT_ERROR":
      message = "H3 导入阶段失败，请检查工作流运行包。";
      break;
    case "PACKAGE_QUEUE_ERROR":
      message = "生产队列创建失败，请查看技术详情后重试。";
      break;
    case "PACKAGE_ITEMS_ALREADY_CREATED":
      message = "所选生产包项目已经创建过生产批次，请重新检查并选择未创建项目。";
      break;
    case "PACKAGE_PROJECT_WORKFLOW_CHANGED":
      message = "项目工作流配置在检查后发生变化，请重新检查生产包。";
      break;
    case "PACKAGE_FILESYSTEM_ERROR":
      message = "生产包文件访问失败，请检查文件权限后重试。";
      break;
    case "PACKAGE_SESSION_EXPIRED":
    case "PACKAGE_SESSION_NOT_FOUND":
      message = "生产包检查结果已过期，请重新检查文件夹后再试。";
      break;
    case "PACKAGE_MEDIA_CHANGED":
      message = "检查后的媒体文件已变化，不能使用旧结果创建批次；请重新检查文件夹。";
      break;
    case "PACKAGE_PROMPT_CHANGED":
    case "PACKAGE_MODE_CHANGED":
    case "PACKAGE_ITEM_NOT_FOUND":
      message = "生产包内容在检查后发生变化，请重新检查文件夹后再试。";
      break;
    case "PACKAGE_ITEM_BLOCKED":
      message = "所选项目当前已被阻止，请重新检查并取消不可生产项目。";
      break;
    default:
      message = `${operation === "inspect" ? "生产包检查失败" : operation === "create" ? "创建生产批次失败" : "打开生产队列失败"}：${toUserMessage(error)}`;
  }
  return {
    message,
    code,
    technicalMessage,
    details,
    requiresReinspect: Boolean(details?.requiresReinspect || (code && STALE_PACKAGE_ERROR_CODES.has(code))),
  };
}
