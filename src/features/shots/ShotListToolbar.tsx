import type { ShotListControls, ShotListStatusFilter } from "./shotListQuery";
import { SHOT_LIST_PAGE_SIZES, SHOT_LIST_STATUS_OPTIONS } from "./shotListQuery";

interface Props {
  controls: ShotListControls;
  filteredCount: number;
  totalCount: number;
  pageStart: number;
  pageEnd: number;
  pageCount: number;
  onQueryChange: (query: string) => void;
  onStatusChange: (status: ShotListStatusFilter) => void;
  onPageSizeChange: (pageSize: number) => void;
  onPageChange: (page: number) => void;
}

export function ShotListToolbar({
  controls,
  filteredCount,
  totalCount,
  pageStart,
  pageEnd,
  pageCount,
  onQueryChange,
  onStatusChange,
  onPageSizeChange,
  onPageChange,
}: Props) {
  return (
    <div className="shot-list-controls">
      <div className="shot-list-control-row">
        <label className="shot-list-search">
          <span className="sr-only">搜索镜头</span>
          <input
            type="search"
            value={controls.query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="搜索镜头名称或 Prompt"
            aria-label="搜索镜头名称或 Prompt"
          />
        </label>
        <label>
          <span className="sr-only">状态筛选</span>
          <select
            value={controls.status}
            onChange={(event) => onStatusChange(event.target.value as ShotListStatusFilter)}
            aria-label="状态筛选"
          >
            {SHOT_LIST_STATUS_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
          </select>
        </label>
        <label>
          <span className="sr-only">每页数量</span>
          <select
            value={controls.pageSize}
            onChange={(event) => onPageSizeChange(Number(event.target.value))}
            aria-label="每页数量"
          >
            {SHOT_LIST_PAGE_SIZES.map((size) => <option key={size} value={size}>{size} / 页</option>)}
          </select>
        </label>
      </div>
      <div className="shot-list-pagination" aria-label="镜头列表分页">
        <span>{pageStart ? `显示 ${pageStart}-${pageEnd}` : "显示 0"} / 匹配 {filteredCount} / 总计 {totalCount}</span>
        <button type="button" className="quiet-button" onClick={() => onPageChange(controls.page - 1)} disabled={controls.page <= 1}>上一页</button>
        <span>第 {controls.page} / {pageCount} 页</span>
        <button type="button" className="quiet-button" onClick={() => onPageChange(controls.page + 1)} disabled={controls.page >= pageCount}>下一页</button>
      </div>
    </div>
  );
}
