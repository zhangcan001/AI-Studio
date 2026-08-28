import { useEffect, useId, useMemo, useState } from "react";
import type {
  ProductionPackageDuration,
  ProductionPackageInspection,
  ProductionPackageInspectionItem,
  ProductionPackageIssue,
  ProductionPackageMedia,
  ProductionPackageResolution,
} from "../../types/productionPackage";

export const PRODUCTION_PACKAGE_PROMPT_PREVIEW_LIMIT = 300;
export const PRODUCTION_PACKAGE_PREVIEW_PAGE_SIZE = 50;

export interface ProductionPackagePreviewProps {
  inspection?: ProductionPackageInspection | null;
  /** Legacy single-item notification kept for existing Preview hosts. */
  onSelectItem?: (item: ProductionPackageInspectionItem) => void;
  /** Optional controlled selection for a future Workspace host. */
  selectedItemIds?: Iterable<string>;
  /** Receives a fresh Set whenever the selection changes. */
  onSelectionChange?: (selectedItemIds: Set<string>) => void;
  /** Defaults to 50; the built-in selector also supports 100. */
  pageSize?: 50 | 100;
}

type StatusFilter = "ALL" | "READY" | "WARNING" | "BLOCKED";
type PreviewPageSize = 50 | 100;

const EMPTY_ITEMS: ProductionPackageInspectionItem[] = [];
const statusFilters: Array<{ value: StatusFilter; label: string }> = [
  { value: "ALL", label: "全部" },
  { value: "READY", label: "READY" },
  { value: "WARNING", label: "WARNING" },
  { value: "BLOCKED", label: "BLOCKED" },
];
const pageSizeOptions: PreviewPageSize[] = [50, 100];

export function ProductionPackagePreview({
  inspection,
  onSelectItem,
  selectedItemIds,
  onSelectionChange,
  pageSize = PRODUCTION_PACKAGE_PREVIEW_PAGE_SIZE,
}: ProductionPackagePreviewProps) {
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("ALL");
  const [page, setPage] = useState(1);
  const normalizedPageSize = normalizePageSize(pageSize);
  const [currentPageSize, setCurrentPageSize] = useState<PreviewPageSize>(normalizedPageSize);
  const [internalSelectedItemIds, setInternalSelectedItemIds] = useState<Set<string>>(
    () => readySelection(inspection?.items ?? EMPTY_ITEMS),
  );
  const pageSizeId = useId();
  const isControlledSelection = selectedItemIds !== undefined;
  const controlledSelection = useMemo(
    () => selectedItemIds === undefined ? undefined : new Set(selectedItemIds),
    [selectedItemIds],
  );
  const items = inspection?.items ?? EMPTY_ITEMS;
  const inspectionSignature = useMemo(
    () => items.map((item) => `${item.id}\u0000${item.status}`).join("\u0001"),
    [items],
  );
  const selection = controlledSelection ?? internalSelectedItemIds;

  useEffect(() => {
    setStatusFilter("ALL");
    setPage(1);
    if (!isControlledSelection) {
      setInternalSelectedItemIds(readySelection(items));
    }
  }, [inspectionSignature, isControlledSelection]);

  useEffect(() => {
    setCurrentPageSize((current) => current === normalizedPageSize ? current : normalizedPageSize);
    setPage(1);
  }, [normalizedPageSize]);

  const counts = useMemo(() => summarizeItems(items), [items]);
  const filteredItems = useMemo(
    () => statusFilter === "ALL" ? items : items.filter((item) => item.status === statusFilter),
    [items, statusFilter],
  );
  const pageCount = Math.max(1, Math.ceil(filteredItems.length / currentPageSize));
  const visiblePage = Math.min(page, pageCount);
  const visibleItems = useMemo(
    () => filteredItems.slice((visiblePage - 1) * currentPageSize, visiblePage * currentPageSize),
    [currentPageSize, filteredItems, visiblePage],
  );
  const readyItems = useMemo(
    () => items.filter((item) => item.status === "READY"),
    [items],
  );
  const selectedItems = useMemo(
    () => items.filter((item) => isSelectableItem(item) && selection.has(item.id)),
    [items, selection],
  );
  const selectedReadyCount = selectedItems.filter((item) => item.status === "READY").length;
  const selectedWarningCount = selectedItems.filter((item) => item.status === "WARNING").length;
  const tableStart = filteredItems.length === 0 ? 0 : (visiblePage - 1) * currentPageSize + 1;
  const tableEnd = Math.min(visiblePage * currentPageSize, filteredItems.length);

  useEffect(() => {
    if (page > pageCount) setPage(pageCount);
  }, [page, pageCount]);

  function commitSelection(nextSelection: Set<string>, item?: ProductionPackageInspectionItem) {
    const next = new Set(nextSelection);
    if (!isControlledSelection) setInternalSelectedItemIds(next);
    onSelectionChange?.(new Set(next));
    if (item) onSelectItem?.(item);
  }

  function toggleItem(item: ProductionPackageInspectionItem) {
    if (!isSelectableItem(item)) return;
    const next = new Set(selection);
    if (next.has(item.id)) next.delete(item.id);
    else next.add(item.id);
    commitSelection(next, item);
  }

  function selectLegacyItem(item: ProductionPackageInspectionItem) {
    if (!isSelectableItem(item)) return;
    const next = new Set(selection);
    next.add(item.id);
    commitSelection(next, item);
  }

  function selectAllReady() {
    commitSelection(new Set(readyItems.map((item) => item.id)));
  }

  function clearSelection() {
    commitSelection(new Set());
  }

  function changeStatusFilter(nextFilter: StatusFilter) {
    setStatusFilter(nextFilter);
    setPage(1);
  }

  function changePageSize(nextPageSize: string) {
    const resolvedPageSize: PreviewPageSize = nextPageSize === "100" ? 100 : 50;
    setCurrentPageSize(resolvedPageSize);
    setPage(1);
  }

  return (
    <section
      className="workspace-panel production-package-preview"
      aria-label="生产包预览"
      data-selected-count={selectedItems.length}
    >
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">External Production Package V1</span>
          <h2>{inspection?.packageName || "Production Package 预览"}</h2>
          <p className="section-description">检查生产包项目状态、媒体元数据和视频提示词；完整提示词不会在预览中展开。</p>
        </div>
        {inspection && <span className="status-pill">{inspection.itemCount} 个项目</span>}
      </div>

      {!inspection ? (
        <p className="empty-state" role="status">尚未加载 Production Package 检查结果。</p>
      ) : (
        <>
          <PackageSummary inspection={inspection} counts={counts} />

          <div className="filter-row" role="group" aria-label="生产包状态筛选">
            {statusFilters.map((filter) => (
              <button
                type="button"
                key={filter.value}
                className={statusFilter === filter.value ? "filter-button filter-button-active" : "filter-button"}
                aria-pressed={statusFilter === filter.value}
                onClick={() => changeStatusFilter(filter.value)}
              >
                {filter.label} {filterCount(counts, filter.value)}
              </button>
            ))}
          </div>

          <div className="production-package-preview-toolbar" role="toolbar" aria-label="生产包预览选择工具">
            <div className="production-package-preview-selection-actions">
              <button type="button" className="quiet-button" onClick={selectAllReady} disabled={readyItems.length === 0}>
                全选 READY
              </button>
              <button type="button" className="quiet-button" onClick={clearSelection} disabled={selectedItems.length === 0}>
                清空选择
              </button>
            </div>
            <label htmlFor={pageSizeId}>每页显示</label>
            <select
              id={pageSizeId}
              aria-label="每页显示项目数"
              value={currentPageSize}
              onChange={(event) => changePageSize(event.target.value)}
            >
              {pageSizeOptions.map((option) => <option key={option} value={option}>{option}</option>)}
            </select>
          </div>

          <p className="production-package-preview-selection-summary" role="status" aria-live="polite">
            已选择 {selectedItems.length} 项（READY {selectedReadyCount}，WARNING {selectedWarningCount}）；READY 默认选中，WARNING 需手动选择，BLOCKED 不可选。
          </p>

          <div className="production-package-preview-table-wrap" style={tableWrapStyle}>
            <table
              aria-label="生产包项目列表"
              aria-describedby={`${pageSizeId}-table-help`}
              style={tableStyle}
            >
              <thead>
                <tr>
                  <th scope="col">选择</th>
                  <th scope="col">状态</th>
                  <th scope="col">ID</th>
                  <th scope="col">名称</th>
                  <th scope="col">模式</th>
                  <th scope="col">图片</th>
                  <th scope="col">首帧</th>
                  <th scope="col">末帧</th>
                  <th scope="col">参考图数量</th>
                  <th scope="col">Video Prompt</th>
                  <th scope="col">时长</th>
                  <th scope="col">分辨率</th>
                  <th scope="col">错误</th>
                </tr>
              </thead>
              <tbody>
                {visibleItems.map((item) => (
                  <InspectionRow
                    key={item.id}
                    item={item}
                    selected={selection.has(item.id)}
                    onToggle={toggleItem}
                    onLegacySelect={onSelectItem ? selectLegacyItem : undefined}
                  />
                ))}
              </tbody>
            </table>
          </div>

          {!visibleItems.length && <p className="empty-state" role="status">当前筛选条件下没有生产包项目。</p>}

          <div id={`${pageSizeId}-table-help`} className="production-package-preview-pagination" role="navigation" aria-label="生产包项目分页">
            <span role="status">
              显示 {visibleItems.length} / {items.length} 个项目；当前页 {tableStart}–{tableEnd} / {filteredItems.length} 项 · 每页 {currentPageSize}
            </span>
            <div className="production-package-preview-page-controls">
              <button
                type="button"
                className="quiet-button"
                aria-label="上一页"
                onClick={() => setPage((current) => Math.max(1, current - 1))}
                disabled={visiblePage <= 1}
              >
                上一页
              </button>
              <span aria-label={`第 ${visiblePage} / ${pageCount} 页`}>第 {visiblePage} / {pageCount} 页</span>
              <button
                type="button"
                className="quiet-button"
                aria-label="下一页"
                onClick={() => setPage((current) => Math.min(pageCount, current + 1))}
                disabled={visiblePage >= pageCount}
              >
                下一页
              </button>
            </div>
          </div>
        </>
      )}
    </section>
  );
}

function PackageSummary({
  inspection,
  counts,
}: {
  inspection: ProductionPackageInspection;
  counts: ItemCounts;
}) {
  const stats = [
    { label: "项目数", value: inspection.itemCount },
    { label: "READY", value: counts.ready },
    { label: "WARNING", value: counts.warning },
    { label: "BLOCKED", value: counts.blocked },
  ];

  return (
    <dl
      className="production-package-preview-summary"
      aria-label="生产包统计"
      style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(120px, 1fr))", gap: 8, margin: "0 0 18px" }}
    >
      {stats.map((stat) => (
        <div key={stat.label} style={{ padding: "10px 12px", border: "1px solid var(--studio-border, #2c3547)", borderRadius: 8 }}>
          <dt style={{ color: "var(--studio-text-secondary, #9ca3af)", fontSize: "0.75rem" }}>{stat.label}</dt>
          <dd style={{ margin: "4px 0 0", fontSize: "1.25rem", fontWeight: 700 }}>{stat.value}</dd>
        </div>
      ))}
    </dl>
  );
}

function InspectionRow({
  item,
  selected,
  onToggle,
  onLegacySelect,
}: {
  item: ProductionPackageInspectionItem;
  selected: boolean;
  onToggle: (item: ProductionPackageInspectionItem) => void;
  onLegacySelect?: (item: ProductionPackageInspectionItem) => void;
}) {
  const selectionDescriptionId = useId();
  const selectable = isSelectableItem(item);
  const selectionDescription = itemSelectionDescription(item.status);
  const issues = [...(item.errors ?? []), ...(item.warnings ?? [])];
  const references = referenceMedia(item);
  const prompt = item.videoPromptPreview ?? item.videoPrompt;
  const imageSummaryText = imageSummary(item, references);

  return (
    <tr data-item-id={item.id} data-status={item.status}>
      <td style={cellStyle}>
        <input
          type="checkbox"
          checked={selectable && selected}
          disabled={!selectable}
          aria-label={`选择项目 ${item.id}`}
          aria-description={selectionDescription}
          aria-describedby={selectionDescriptionId}
          aria-disabled={!selectable}
          onChange={() => onToggle(item)}
        />
        <span id={selectionDescriptionId} style={visuallyHiddenStyle}>{selectionDescription}</span>
      </td>
      <td style={cellStyle}>
        <StatusBadge status={item.status} />
        <small style={secondaryLineStyle}>{selectionDescription}</small>
      </td>
      <td style={cellStyle}>
        {onLegacySelect ? (
          <button
            type="button"
            className="quiet-button production-package-preview-item-button"
            aria-label={`选择项目 ${item.id}`}
            aria-pressed={selectable && selected}
            disabled={!selectable}
            onClick={() => onLegacySelect(item)}
          >
            {item.id}
          </button>
        ) : <code>{item.id}</code>}
      </td>
      <td style={cellStyle}>{item.name || "—"}</td>
      <td style={cellStyle}>{item.mode || "—"}</td>
      <td style={cellStyle} aria-label={`图片：${imageSummaryText}`}>见首帧、末帧和参考图数量列</td>
      <td style={cellStyle}>{item.firstFrame ? `首帧：${mediaLabel(item.firstFrame)}` : "无首帧"}</td>
      <td style={cellStyle}>{item.lastFrame ? `末帧：${mediaLabel(item.lastFrame)}` : "无末帧"}</td>
      <td style={cellStyle}>参考图：{references.length} 张</td>
      <td style={{ ...cellStyle, maxWidth: 320, overflowWrap: "anywhere" }} title="仅显示最多 300 个字符的预览">
        {truncateProductionPackagePrompt(prompt)}
      </td>
      <td style={cellStyle}>{formatDuration(item.duration ?? item.durationSeconds)}</td>
      <td style={cellStyle}>{formatResolution(item.resolution, item.width, item.height)}</td>
      <td style={cellStyle}>
        {issues.length ? <IssueList itemId={item.id} issues={issues} /> : "—"}
      </td>
    </tr>
  );
}

function StatusBadge({ status }: { status: string }) {
  const normalized = status.toLowerCase().replace(/[^a-z0-9_-]/g, "-");
  return (
    <span
      className={`status-pill production-package-preview-status-${normalized}`}
      data-status={status}
      title={`状态：${status}`}
    >
      {status}
    </span>
  );
}

function IssueList({ itemId, issues }: { itemId: string; issues: ProductionPackageIssue[] }) {
  return (
    <ul aria-label={`项目 ${itemId} 的问题`} style={{ margin: 0, paddingLeft: 16 }}>
      {issues.map((issue, index) => <li key={`${issueText(issue)}-${index}`}>{issueText(issue)}</li>)}
    </ul>
  );
}

export function truncateProductionPackagePrompt(
  value: string | null | undefined,
  limit = PRODUCTION_PACKAGE_PROMPT_PREVIEW_LIMIT,
): string {
  const text = value?.trim() ?? "";
  if (!text) return "—";
  const characters = Array.from(text);
  const normalizedLimit = Math.max(0, Math.floor(limit));
  return characters.length > normalizedLimit ? `${characters.slice(0, normalizedLimit).join("")}…` : text;
}

interface ItemCounts {
  all: number;
  ready: number;
  warning: number;
  blocked: number;
}

function summarizeItems(items: ProductionPackageInspectionItem[]): ItemCounts {
  return items.reduce<ItemCounts>((counts, item) => {
    counts.all += 1;
    if (item.status === "READY") counts.ready += 1;
    else if (item.status === "WARNING") counts.warning += 1;
    else if (item.status === "BLOCKED") counts.blocked += 1;
    return counts;
  }, { all: 0, ready: 0, warning: 0, blocked: 0 });
}

function filterCount(counts: ItemCounts, filter: StatusFilter): number {
  switch (filter) {
    case "READY": return counts.ready;
    case "WARNING": return counts.warning;
    case "BLOCKED": return counts.blocked;
    case "ALL": return counts.all;
  }
}

function normalizePageSize(value: number): PreviewPageSize {
  return value === 100 ? 100 : 50;
}

function readySelection(items: ProductionPackageInspectionItem[]): Set<string> {
  return new Set(items.filter((item) => item.status === "READY").map((item) => item.id));
}

function isSelectableItem(item: ProductionPackageInspectionItem): boolean {
  return item.status === "READY" || item.status === "WARNING";
}

function itemSelectionDescription(status: string): string {
  if (status === "READY") return "READY 项目默认选中，可取消选择。";
  if (status === "WARNING") return "WARNING 项目需手动选择。";
  if (status === "BLOCKED") return "BLOCKED 项目不可选择。";
  return `${status} 项目不可选择。`;
}

function referenceMedia(item: ProductionPackageInspectionItem): ProductionPackageMedia[] {
  if (item.references) return item.references;
  return [
    ...(item.referenceImages ?? []),
    ...(item.referenceAudios ?? []),
    ...(item.referenceVideos ?? []),
  ];
}

function mediaLabel(media: ProductionPackageMedia): string {
  if (typeof media === "string") return media;
  return media.relativePath || media.path || media.fileName || media.displayName || "已提供";
}

function imageSummary(item: ProductionPackageInspectionItem, references: ProductionPackageMedia[]): string {
  const parts = [
    item.firstFrame ? `首帧：${mediaLabel(item.firstFrame)}` : "无首帧",
    item.lastFrame ? `末帧：${mediaLabel(item.lastFrame)}` : "无末帧",
    `参考图：${references.length} 张`,
  ];
  return parts.join("；");
}

function formatDuration(value: ProductionPackageDuration | null | undefined): string {
  if (value === undefined || value === null || value === "") return "—";
  if (typeof value === "number") return `${value} 秒`;
  if (typeof value === "string") return value;
  const seconds = value.seconds ?? value.durationSeconds;
  return seconds === undefined || seconds === null ? "—" : `${seconds} 秒`;
}

function formatResolution(
  value: ProductionPackageResolution | string | null | undefined,
  width?: number | null,
  height?: number | null,
): string {
  if (typeof value === "string" && value.trim()) return value;
  const resolvedWidth = typeof value === "object" && value ? value.width : width;
  const resolvedHeight = typeof value === "object" && value ? value.height : height;
  return resolvedWidth != null && resolvedHeight != null ? `${resolvedWidth} × ${resolvedHeight}` : "—";
}

function issueText(issue: ProductionPackageIssue): string {
  if (typeof issue === "string") return issue;
  return [issue.code, issue.message, issue.detail].filter(Boolean).join("：") || "未知问题";
}

const tableWrapStyle = {
  maxHeight: "min(62vh, 560px)",
  overflow: "auto",
  overscrollBehavior: "contain",
} as const;

const tableStyle = {
  width: "100%",
  minWidth: 1480,
  borderCollapse: "collapse",
} as const;

const cellStyle = {
  padding: "9px 8px",
  borderBottom: "1px solid var(--studio-border, #2c3547)",
  textAlign: "left",
  verticalAlign: "top",
} as const;

const secondaryLineStyle = {
  display: "block",
  color: "var(--studio-text-secondary, #9ca3af)",
  fontSize: "0.75rem",
} as const;

const visuallyHiddenStyle = {
  position: "absolute",
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: "hidden",
  clip: "rect(0, 0, 0, 0)",
  whiteSpace: "nowrap",
  border: 0,
} as const;
