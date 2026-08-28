import { useMemo, useState } from "react";
import type {
  ProductionPackageDuration,
  ProductionPackageInspection,
  ProductionPackageInspectionItem,
  ProductionPackageIssue,
  ProductionPackageMedia,
  ProductionPackageResolution,
} from "../../types/productionPackage";

export const PRODUCTION_PACKAGE_PROMPT_PREVIEW_LIMIT = 300;

export interface ProductionPackagePreviewProps {
  inspection?: ProductionPackageInspection | null;
  onSelectItem?: (item: ProductionPackageInspectionItem) => void;
}

type StatusFilter = "ALL" | "READY" | "WARNING" | "BLOCKED";

const statusFilters: Array<{ value: StatusFilter; label: string }> = [
  { value: "ALL", label: "全部" },
  { value: "READY", label: "READY" },
  { value: "WARNING", label: "WARNING" },
  { value: "BLOCKED", label: "BLOCKED" },
];

export function ProductionPackagePreview({ inspection, onSelectItem }: ProductionPackagePreviewProps) {
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("ALL");
  const [selectedItemId, setSelectedItemId] = useState<string>();
  const items = inspection?.items ?? [];
  const visibleItems = useMemo(
    () => statusFilter === "ALL" ? items : items.filter((item) => item.status === statusFilter),
    [items, statusFilter],
  );

  function selectItem(item: ProductionPackageInspectionItem) {
    setSelectedItemId(item.id);
    onSelectItem?.(item);
  }

  return (
    <section className="workspace-panel production-package-preview" aria-label="生产包预览">
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
          <PackageSummary inspection={inspection} />

          <div className="filter-row" role="group" aria-label="生产包状态筛选">
            {statusFilters.map((filter) => (
              <button
                type="button"
                key={filter.value}
                className={statusFilter === filter.value ? "filter-button filter-button-active" : "filter-button"}
                aria-pressed={statusFilter === filter.value}
                onClick={() => setStatusFilter(filter.value)}
              >
                {filter.label} {filterCount(inspection, filter.value)}
              </button>
            ))}
          </div>

          <div className="production-package-preview-table-wrap" style={{ overflowX: "auto" }}>
            <table aria-label="生产包项目列表" style={{ width: "100%", minWidth: 980, borderCollapse: "collapse" }}>
              <thead>
                <tr>
                  <th scope="col">状态</th>
                  <th scope="col">ID</th>
                  <th scope="col">名称</th>
                  <th scope="col">模式</th>
                  <th scope="col">图片</th>
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
                    selected={selectedItemId === item.id}
                    onSelect={onSelectItem ? selectItem : undefined}
                  />
                ))}
              </tbody>
            </table>
          </div>
          {!visibleItems.length && <p className="empty-state" role="status">当前筛选条件下没有生产包项目。</p>}
          <p className="production-package-preview-caption" role="status">
            显示 {visibleItems.length} / {items.length} 个项目；预览只读，不会创建批次或启动生产。
          </p>
        </>
      )}
    </section>
  );
}

function PackageSummary({ inspection }: { inspection: ProductionPackageInspection }) {
  const stats = [
    { label: "项目数", value: inspection.itemCount },
    { label: "READY", value: inspection.readyCount },
    { label: "WARNING", value: inspection.warningCount },
    { label: "BLOCKED", value: inspection.blockedCount },
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
  onSelect,
}: {
  item: ProductionPackageInspectionItem;
  selected: boolean;
  onSelect?: (item: ProductionPackageInspectionItem) => void;
}) {
  const issues = [...(item.errors ?? []), ...(item.warnings ?? [])];
  const references = item.references ?? item.referenceImages ?? [];

  return (
    <tr data-item-id={item.id} data-status={item.status}>
      <td style={cellStyle}><StatusBadge status={item.status} /></td>
      <td style={cellStyle}>
        {onSelect ? (
          <button
            type="button"
            className="quiet-button production-package-preview-item-button"
            aria-label={`选择项目 ${item.id}`}
            aria-pressed={selected}
            onClick={() => onSelect(item)}
          >
            {item.id}
          </button>
        ) : <code>{item.id}</code>}
      </td>
      <td style={cellStyle}>{item.name || "—"}</td>
      <td style={cellStyle}>{item.mode || "—"}</td>
      <td style={cellStyle}>
        <div aria-label={`图片：${imageSummary(item, references)}`}>
          <span>{item.firstFrame ? `首帧：${mediaLabel(item.firstFrame)}` : "无首帧"}</span>
          {item.lastFrame && <span style={secondaryLineStyle}>末帧：{mediaLabel(item.lastFrame)}</span>}
          {references.length > 0 && <span style={secondaryLineStyle}>参考图：{references.length} 张</span>}
        </div>
      </td>
      <td style={{ ...cellStyle, maxWidth: 320 }}>
        <span title="仅显示最多 300 个字符的预览">{truncateProductionPackagePrompt(item.videoPromptPreview)}</span>
      </td>
      <td style={cellStyle}>{formatDuration(item.duration ?? item.durationSeconds)}</td>
      <td style={cellStyle}>{formatResolution(item.resolution, item.width, item.height)}</td>
      <td style={cellStyle}>{issues.length ? <IssueList issues={issues} /> : "—"}</td>
    </tr>
  );
}

function StatusBadge({ status }: { status: string }) {
  const normalized = status.toLowerCase().replace(/[^a-z0-9_-]/g, "-");
  return <span className={`status-pill production-package-preview-status-${normalized}`}>{status}</span>;
}

function IssueList({ issues }: { issues: ProductionPackageIssue[] }) {
  return (
    <ul aria-label="生产包问题" style={{ margin: 0, paddingLeft: 16 }}>
      {issues.map((issue, index) => <li key={`${issueText(issue)}-${index}`}>{issueText(issue)}</li>)}
    </ul>
  );
}

export function truncateProductionPackagePrompt(value: string | null | undefined, limit = PRODUCTION_PACKAGE_PROMPT_PREVIEW_LIMIT): string {
  const text = value?.trim() ?? "";
  if (!text) return "—";
  const characters = Array.from(text);
  return characters.length > limit ? `${characters.slice(0, limit).join("")}…` : text;
}

function filterCount(inspection: ProductionPackageInspection, filter: StatusFilter): number {
  if (filter === "ALL") return inspection.itemCount;
  return filter === "READY" ? inspection.readyCount : filter === "WARNING" ? inspection.warningCount : inspection.blockedCount;
}

function mediaLabel(media: ProductionPackageMedia): string {
  if (typeof media === "string") return media;
  return media.relativePath || media.path || media.fileName || media.displayName || "已提供";
}

function imageSummary(item: ProductionPackageInspectionItem, references: ProductionPackageMedia[]): string {
  const parts = [item.firstFrame && mediaLabel(item.firstFrame), item.lastFrame && mediaLabel(item.lastFrame)];
  if (references.length) parts.push(`${references.length} 张参考图`);
  return parts.filter(Boolean).join("；") || "未提供图片";
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
  return resolvedWidth && resolvedHeight ? `${resolvedWidth} × ${resolvedHeight}` : "—";
}

function issueText(issue: ProductionPackageIssue): string {
  if (typeof issue === "string") return issue;
  return [issue.code, issue.message, issue.detail].filter(Boolean).join("：") || "未知问题";
}

const cellStyle = { padding: "9px 8px", borderBottom: "1px solid var(--studio-border, #2c3547)", textAlign: "left", verticalAlign: "top" } as const;
const secondaryLineStyle = { display: "block", color: "var(--studio-text-secondary, #9ca3af)", fontSize: "0.75rem" } as const;
