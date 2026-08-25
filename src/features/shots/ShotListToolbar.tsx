import { useState } from "react";
import type { ShotListControls, ShotListStatusFilter } from "./shotListQuery";
import { DEFAULT_SHOT_LIST_PAGE_SIZE, SHOT_LIST_PAGE_SIZES, SHOT_LIST_STATUS_OPTIONS } from "./shotListQuery";

interface Props {
  controls: ShotListControls;
  filteredCount: number;
  totalCount: number;
  pageStart: number;
  pageEnd: number;
  pageCount: number;
  onQueryChange: (query: string) => void;
  onStatusChange: (status: ShotListStatusFilter) => void;
  sceneOptions?: ReadonlyArray<{ value: string; label: string }>;
  onSceneChange?: (sceneId: string) => void;
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
  sceneOptions = [],
  onSceneChange,
  onPageSizeChange,
  onPageChange,
}: Props) {
  const [filterOpen, setFilterOpen] = useState(false);
  const filterActive = controls.status !== "ALL" || controls.sceneId !== "ALL" || controls.pageSize !== DEFAULT_SHOT_LIST_PAGE_SIZE;
  return (
    <div className="shot-list-controls">
      <div className="shot-list-control-row">
        <label className="shot-list-search">
          <span className="sr-only">搜索镜头</span>
          <input
            type="search"
            value={controls.query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="搜索镜头名称或提示词"
            aria-label="搜索镜头名称或提示词"
          />
        </label>
        <button type="button" className={filterActive ? "shot-list-filter-trigger shot-list-filter-trigger-active" : "shot-list-filter-trigger"} aria-label="镜头筛选" aria-controls="shot-list-filter-popover" aria-expanded={filterOpen} onClick={() => setFilterOpen((open) => !open)}>
          <svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><path d="M4 6h16M7 12h10M10 18h4" /></svg>
          <span>筛选</span>
        </button>
      </div>
      <div id="shot-list-filter-popover" className={filterOpen ? "shot-list-filter-popover shot-list-filter-popover-open" : "shot-list-filter-popover"} hidden={!filterOpen}>
        <label>
          <span>状态</span>
          <select value={controls.status} onChange={(event) => onStatusChange(event.target.value as ShotListStatusFilter)} aria-label="状态筛选">
            {SHOT_LIST_STATUS_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
          </select>
        </label>
        {sceneOptions.length > 0 && onSceneChange && <label>
          <span>场景</span>
          <select value={controls.sceneId} onChange={(event) => onSceneChange(event.target.value)} aria-label="结构筛选">
            {sceneOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
          </select>
        </label>}
        <label>
          <span>每页数量</span>
          <select value={controls.pageSize} onChange={(event) => onPageSizeChange(Number(event.target.value))} aria-label="每页数量">
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
