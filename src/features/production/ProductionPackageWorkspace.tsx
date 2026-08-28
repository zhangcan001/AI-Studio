import { useEffect, useMemo, useRef, useState } from "react";
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
  /** The queue remains a manual gate; this callback is never invoked after create automatically. */
  onOpenQueue?: () => void | Promise<void>;
  /** Optional richer queue callback for hosts that need the created batch mapping. */
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
  requiresReinspect: boolean;
}

const STALE_PACKAGE_ERROR_CODES = new Set([
  "PACKAGE_SESSION_EXPIRED",
  "PACKAGE_SESSION_NOT_FOUND",
  "PACKAGE_MEDIA_CHANGED",
  "PACKAGE_PROMPT_CHANGED",
  "PACKAGE_MODE_CHANGED",
  "PACKAGE_ITEM_NOT_FOUND",
  "PACKAGE_ITEM_BLOCKED",
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
  const [error, setError] = useState<WorkspaceError>();
  const [notice, setNotice] = useState<string>();
  const requestIdRef = useRef(0);
  const observedFolderKeyRef = useRef<string | undefined>(undefined);
  const preferredSelectionIdsRef = useRef<Set<string> | undefined>(undefined);

  const pickFolder = onChooseFolder ?? onPickFolder ?? onSelectFolder ?? onOpenFolderPicker ?? folderPicker;
  const busy = isInspecting || isPickingFolder || isCreating || isOpeningQueue;

  function resetWorkspace() {
    requestIdRef.current += 1;
    setInspection(undefined);
    setSelectedItemIds(new Set());
    setCreatedResult(undefined);
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
  const workspaceState: ProductionPackageWorkspaceState = isInspecting || isPickingFolder
    ? "INSPECTING"
    : isCreating
      ? "CREATING_BATCHES"
      : error
        ? "ERROR"
        : createdResult
          ? isPartialCreate ? "PARTIAL" : "CREATED"
          : derivedInspectionState;
  const hasQueueCallback = Boolean(onOpenQueue || onOpenProductionQueue);

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
      const createdCount = result.createdCount ?? result.itemCount;
      const remainingCount = result.remainingCount ?? result.remainingItemIds?.length ?? 0;
      setNotice(result.status === "PARTIAL"
        ? `批次创建部分完成：已加入 ${createdCount} 个项目，尚未加入 ${remainingCount} 个项目；不会自动启动队列。`
        : `已创建 ${result.batchCount} 个生产批次，共 ${createdCount} 个项目；不会自动启动队列。`);
    } catch (createError: unknown) {
      setError(toWorkspaceError(createError, "create"));
    } finally {
      setIsCreating(false);
    }
  }

  async function openQueue() {
    if (!createdResult || !hasQueueCallback || isOpeningQueue) return;
    setIsOpeningQueue(true);
    setError(undefined);
    try {
      if (onOpenQueue) await onOpenQueue();
      else await onOpenProductionQueue?.(createdResult);
      setNotice("已请求打开生产队列；队列仍需由用户手动启动。 ");
    } catch (openError: unknown) {
      setError(toWorkspaceError(openError, "open"));
    } finally {
      setIsOpeningQueue(false);
    }
  }

  const statusMessage = createdResult
    ? isPartialCreate
      ? `批次创建部分完成：已加入 ${createdCount} 个项目，尚有 ${remainingCount} 个项目待重新检查。`
      : "批次已创建；不会自动打开或启动生产队列。"
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
          <p className="section-description">选择由外部智能体准备好的 Production Package 文件夹；确认项目后只创建批次，不自动启动生产。</p>
        </div>
        <span className={`production-package-workspace-state production-package-workspace-state-${workspaceState.toLowerCase()}`}>
          {workspaceStateLabel(workspaceState)}
        </span>
      </div>

      <div className="production-package-workspace-folder" aria-label="生产包文件夹入口">
        <label htmlFor="production-package-folder-path">Production Package 文件夹路径</label>
        <div className="production-package-workspace-folder-row">
          <input
            id="production-package-folder-path"
            type="text"
            value={currentFolderPath ? displayFolderName(currentFolderPath) : ""}
            placeholder="尚未选择生产包文件夹"
            readOnly
            aria-readonly="true"
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
            <li>创建只会加入现有生产队列，不会自动打开或启动队列。</li>
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
          {currentFolderPath && <small>请点击“重新检查”获取最新检查结果后再继续。</small>}
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

          <p className="production-package-workspace-selection-summary" role="status" aria-live="polite">
            已选择 {selectedItems.length} 项（READY {selectedReadyCount}，WARNING {selectedWarningCount}）；WARNING 需手动选择，BLOCKED 不可选。
          </p>

          <fieldset disabled={busy} className="production-package-workspace-preview-fieldset">
            <ProductionPackagePreview
              inspection={inspection}
              selectedItemIds={selectedItemIds}
              onSelectionChange={handlePreviewSelectionChange}
              pageSize={50}
            />
          </fieldset>
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
          <p>队列不会自动打开或启动，请在需要时手动打开生产队列。</p>
          <ul className="production-package-workspace-batch-list" aria-label="已创建生产批次">
            {createdResult.batches.map((batch, index) => (
              <li key={batch.batchId}>
                <strong>{batch.batchName || `批次 ${index + 1}`}</strong>
                <span>{batch.itemCount} 个项目 · {batch.batchId}</span>
              </li>
            ))}
          </ul>
          {isPartialCreate && remainingCount > 0 && (
            <div className="production-package-workspace-remaining">
              <p>保留 {remainingCount} 个未加入项目的外部 ID；重新检查后只会按最新检查结果选择仍可生产的剩余项目。</p>
              <button type="button" onClick={reinspectRemaining} disabled={busy || !currentFolderPath}>
                重新检查剩余项目
              </button>
            </div>
          )}
          <button type="button" onClick={() => void openQueue()} disabled={!hasQueueCallback || isOpeningQueue}>
            {isOpeningQueue ? "正在打开…" : "打开生产队列"}
          </button>
          {!hasQueueCallback && <small className="production-package-workspace-queue-note">父层尚未接入打开生产队列回调。</small>}
        </section>
      )}

      <div className="production-package-workspace-footer">
        <button
          type="button"
          className="production-package-workspace-create-button"
          onClick={() => void createBatches()}
          disabled={!canCreate}
        >
          {isCreating ? "正在创建批次…" : createdResult ? "批次已创建" : `创建生产批次${selectionIdsForCreate.length ? `（${selectionIdsForCreate.length} 项）` : ""}`}
        </button>
        {workspaceState === "BLOCKED" && <small>没有可创建的 READY/WARNING 项目，请修复生产包后重新检查。</small>}
        {workspaceState === "ERROR" && !requiresReinspect && <small>可先重新检查，也可修复错误后再次创建批次。</small>}
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
    case "EMPTY": return "等待选择文件夹";
    case "INSPECTING": return "正在检查";
    case "READY": return "检查通过";
    case "PARTIAL": return "部分可生产";
    case "BLOCKED": return "无法创建";
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
    case "EMPTY": return "尚未选择 Production Package 文件夹。请选择包含 production-package.json 的目录。";
    case "INSPECTING": return "正在检查 Production Package，请稍候。";
    case "READY": return `检查完成：${counts.total} 个项目全部 READY，可创建批次。`;
    case "PARTIAL": return `检查完成：${counts.ready} 个 READY、${counts.warning} 个 WARNING、${counts.blocked} 个 BLOCKED；当前已选择 ${selectedCount} 项。`;
    case "BLOCKED": return "检查完成，但没有可创建的 READY 或 WARNING 项目。请修复生产包后重新检查。";
    case "CREATING_BATCHES": return `正在创建批次，${selectedCount} 个已选择项目处理中；操作完成前控件已禁用。`;
    case "CREATED": return "批次已创建；不会自动打开或启动生产队列。";
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

function statusErrorCode(error: unknown): string | undefined {
  if (error && typeof error === "object" && "code" in error) {
    const code = (error as { code?: unknown }).code;
    if (typeof code === "string" && code) return code;
  }
  const raw = rawErrorText(error);
  return raw.match(/[A-Z][A-Z0-9_]{2,}/)?.[0];
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
  let message: string;
  switch (code) {
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
    requiresReinspect: Boolean(code && STALE_PACKAGE_ERROR_CODES.has(code)),
  };
}
