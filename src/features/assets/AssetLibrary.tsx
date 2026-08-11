import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { assetLibraryPage, bulkAddAssetTag, bulkRemoveAssetTag, bulkSetAssetFavorite, getAsset, listAssetTags, setAssetFavorite } from "../../services/tauriClient";
import type {
  AssetCategoryFilter,
  AssetCreatedOrder,
  AssetLibraryQuery,
  AssetMediaTypeFilter,
  AssetSourceFilter,
  AssetView,
  PageCursor,
} from "../../types/asset";
import { toUserMessage } from "../../i18n/errorMessages";
import { AssetCompareWorkspace } from "./AssetCompareWorkspace";
import { toggleCompareSelection } from "./assetCompare";
import { mergeAssetPage } from "./assetLibraryState";
import { AssetGrid } from "./AssetGrid";
import { AssetPreview } from "./AssetPreview";
import { AssetDeleteDialog } from "./AssetDeleteDialog";
import { TagManagerDialog } from "./TagManagerDialog";
import type { AssetTag } from "../../types/organization";
import { replaceAssetOrganization } from "./assetOrganization";

const categories: Array<{ value: AssetCategoryFilter; label: string }> = [
  { value: "ALL", label: "全部分类" },
  { value: "SOURCE_IMAGE", label: "源图片" },
  { value: "SOURCE_VIDEO", label: "源视频" },
  { value: "SOURCE_AUDIO", label: "源音频" },
  { value: "GENERATED_IMAGE", label: "生成图片" },
  { value: "GENERATED_VIDEO", label: "生成视频" },
];

interface Props {
  projectId: string;
  onUseInStudio: (asset: AssetView) => void;
  onOpenVideoBatch: (assets: AssetView[]) => void;
  onOpenTask: (taskId: string) => void;
}

export function AssetLibrary({ projectId, onUseInStudio, onOpenVideoBatch, onOpenTask }: Props) {
  const [category, setCategory] = useState<AssetCategoryFilter>("ALL");
  const [keywordInput, setKeywordInput] = useState("");
  const [keyword, setKeyword] = useState("");
  const [mediaType, setMediaType] = useState<AssetMediaTypeFilter>("ALL");
  const [sourceKind, setSourceKind] = useState<AssetSourceFilter>("ALL");
  const [createdOrder, setCreatedOrder] = useState<AssetCreatedOrder>("NEWEST");
  const [favoriteOnly, setFavoriteOnly] = useState(false);
  const [tagId, setTagId] = useState("");
  const [tags, setTags] = useState<AssetTag[]>([]);
  const [tagManagerOpen, setTagManagerOpen] = useState(false);
  const [assets, setAssets] = useState<AssetView[]>([]);
  const [cursor, setCursor] = useState<PageCursor>();
  const [selectedAsset, setSelectedAsset] = useState<AssetView>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [compareMode, setCompareMode] = useState(false);
  const [compareAssets, setCompareAssets] = useState<AssetView[]>([]);
  const [compareOpen, setCompareOpen] = useState(false);
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedAssetIds, setSelectedAssetIds] = useState<Set<string>>(new Set());
  const [bulkTagId, setBulkTagId] = useState("");
  const [bulkBusy, setBulkBusy] = useState(false);
  const [deleteRequest, setDeleteRequest] = useState<AssetView[]>();
  const requestVersion = useRef(0);

  useEffect(() => {
    const timer = window.setTimeout(() => setKeyword(keywordInput.trim()), 300);
    return () => window.clearTimeout(timer);
  }, [keywordInput]);

  const query = useMemo<AssetLibraryQuery>(() => ({
    projectId,
    category,
    keyword: keyword || undefined,
    mediaType,
    sourceKind,
    favoriteOnly,
    tagId: tagId || undefined,
    createdOrder,
    limit: 30,
  }), [category, createdOrder, favoriteOnly, keyword, mediaType, projectId, sourceKind, tagId]);

  const reloadTags = useCallback(async () => {
    const next = await listAssetTags(projectId);
    setTags(next);
    if (tagId && !next.some((tag) => tag.id === tagId)) setTagId("");
  }, [projectId, tagId]);

  useEffect(() => { void reloadTags().catch(() => setTags([])); }, [reloadTags]);

  const requestPage = useCallback(async (requestedCursor: PageCursor | undefined, reset: boolean) => {
    const version = ++requestVersion.current;
    setLoading(true);
    setError(undefined);
    try {
      const page = await assetLibraryPage({ ...query, cursor: requestedCursor });
      if (requestVersion.current !== version) return;
      setAssets((current) => mergeAssetPage(current, page.items, reset));
      setCursor(page.nextCursor);
    } catch (loadError: unknown) {
      if (requestVersion.current === version) setError(toUserMessage(loadError));
    } finally {
      if (requestVersion.current === version) setLoading(false);
    }
  }, [query]);

  useEffect(() => {
    setAssets([]);
    setCursor(undefined);
    setSelectedAsset(undefined);
    setSelectedAssetIds(new Set());
    setError(undefined);
    void requestPage(undefined, true);
    return () => {
      requestVersion.current += 1;
    };
  }, [requestPage]);

  useEffect(() => {
    setCompareMode(false);
    setCompareAssets([]);
    setCompareOpen(false);
  }, [projectId]);

  function clearFilters() {
    setKeywordInput("");
    setKeyword("");
    setCategory("ALL");
    setMediaType("ALL");
    setSourceKind("ALL");
    setFavoriteOnly(false);
    setTagId("");
    setCreatedOrder("NEWEST");
    setNotice(undefined);
  }

  function toggleBulkSelection(asset: AssetView) {
    setSelectedAssetIds((current) => {
      const next = new Set(current);
      if (next.has(asset.id)) next.delete(asset.id);
      else if (next.size < 100) next.add(asset.id);
      else setNotice("批量整理一次最多选择 100 项。");
      return next;
    });
  }

  async function runBulkFavorite(favorite: boolean) {
    if (!selectedAssetIds.size) return;
    setBulkBusy(true); setError(undefined); setNotice(undefined);
    try {
      await bulkSetAssetFavorite(projectId, [...selectedAssetIds], favorite);
      setNotice(`${favorite ? "已收藏" : "已取消收藏"} ${selectedAssetIds.size} 项素材。`);
      setSelectedAssetIds(new Set());
      await requestPage(undefined, true);
    } catch (value: unknown) { setError(toUserMessage(value)); }
    finally { setBulkBusy(false); }
  }

  async function runBulkTag(add: boolean) {
    if (!selectedAssetIds.size || !bulkTagId) return;
    setBulkBusy(true); setError(undefined); setNotice(undefined);
    try {
      if (add) await bulkAddAssetTag(projectId, [...selectedAssetIds], bulkTagId);
      else await bulkRemoveAssetTag(projectId, [...selectedAssetIds], bulkTagId);
      setNotice(`${add ? "已添加" : "已移除"}标签，共 ${selectedAssetIds.size} 项素材。`);
      setSelectedAssetIds(new Set());
      setBulkTagId("");
      await requestPage(undefined, true);
    } catch (value: unknown) { setError(toUserMessage(value)); }
    finally { setBulkBusy(false); }
  }

  async function toggleFavorite(asset: AssetView) {
    try {
      await setAssetFavorite(projectId, asset.id, !asset.isFavorite);
      const refreshed = await getAsset(projectId, asset.id);
      applyOrganizationAsset(refreshed);
      if (favoriteOnly && !refreshed.isFavorite) void requestPage(undefined, true);
    } catch (value) { setError(toUserMessage(value)); }
  }

  function applyOrganizationAsset(refreshed: AssetView) {
    setAssets((current) => replaceAssetOrganization(current, refreshed));
    setSelectedAsset((current) => current?.id === refreshed.id ? refreshed : current);
    setCompareAssets((current) => replaceAssetOrganization(current, refreshed));
    void reloadTags().catch(() => undefined);
  }

  function toggleCompare(asset: AssetView) {
    const result = toggleCompareSelection(compareAssets, asset);
    setCompareAssets(result.assets);
    setNotice(result.notice);
  }

  const hasFilters = Boolean(keyword || category !== "ALL" || mediaType !== "ALL" || sourceKind !== "ALL" || favoriteOnly || tagId);
  const emptyMessage = hasFilters ? "没有找到符合条件的素材。" : "当前项目还没有素材。";
  const selectedVideoAssets = assets.filter((asset) => selectedAssetIds.has(asset.id));

  function requestDeleteSelection() {
    const selected = assets.filter((asset) => selectedAssetIds.has(asset.id));
    if (selected.length) setDeleteRequest(selected);
  }

  function handleDeleted(assetIds: string[], result: { deletedCount: number; warnings: string[] }) {
    const deleted = new Set(assetIds);
    setAssets((current) => current.filter((asset) => !deleted.has(asset.id)));
    setCompareAssets((current) => current.filter((asset) => !deleted.has(asset.id)));
    setSelectedAssetIds((current) => new Set([...current].filter((assetId) => !deleted.has(assetId))));
    setSelectedAsset((current) => (current && deleted.has(current.id) ? undefined : current));
    setCompareOpen((current) => current && compareAssets.some((asset) => deleted.has(asset.id)) ? false : current);
    setDeleteRequest(undefined);
    setNotice(`已删除 ${result.deletedCount} 个素材。${result.warnings.length ? ` ${result.warnings.join(" ")}` : ""}`);
    void requestPage(undefined, true);
  }

  return (
    <section className="workspace-panel" aria-busy={loading}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">资产</span>
          <h2>资产库</h2>
          <p className="section-description">搜索、筛选、对比当前项目的源素材和生成结果。</p>
        </div>
        <div className="asset-library-actions">
          <button type="button" className="quiet-button" onClick={() => setTagManagerOpen(true)}>管理标签</button>
          <button type="button" className={compareMode ? "filter-button filter-button-active" : "quiet-button"} onClick={() => setCompareMode((value) => !value)}>
            {compareMode ? "结束对比选择" : "选择进行对比"}
          </button>
          <button type="button" className={selectionMode ? "filter-button filter-button-active" : "quiet-button"} onClick={() => setSelectionMode((value) => !value)}>
            {selectionMode ? "结束批量整理" : "批量选择"}
          </button>
          <button type="button" className="quiet-button" onClick={() => void requestPage(undefined, true)} disabled={loading}>
            {loading ? "正在刷新..." : "刷新"}
          </button>
        </div>
      </div>

      <div className="asset-library-query" aria-label="资产查询">
        <label className="asset-search-field">
          <span>搜索素材</span>
          <input value={keywordInput} onChange={(event) => setKeywordInput(event.target.value)} placeholder="搜索名称或原始文件名" />
        </label>
        <label>
          <span>来源</span>
          <select value={sourceKind} onChange={(event) => setSourceKind(event.target.value as AssetSourceFilter)}>
            <option value="ALL">全部</option>
            <option value="SOURCE">源素材</option>
            <option value="GENERATED">生成结果</option>
          </select>
        </label>
        <label>
          <span>标签</span>
          <select value={tagId} onChange={(event) => setTagId(event.target.value)}>
            <option value="">全部标签</option>
            {tags.map((tag) => <option key={tag.id} value={tag.id}>{tag.name}</option>)}
          </select>
        </label>
        <label className="check-control asset-favorite-filter">
          <input type="checkbox" checked={favoriteOnly} onChange={(event) => setFavoriteOnly(event.target.checked)} />
          <span>仅收藏</span>
        </label>
        <label>
          <span>类型</span>
          <select value={mediaType} onChange={(event) => setMediaType(event.target.value as AssetMediaTypeFilter)}>
            <option value="ALL">全部类型</option>
            <option value="IMAGE">图片</option>
            <option value="VIDEO">视频</option>
            <option value="AUDIO">音频</option>
          </select>
        </label>
        <label>
          <span>排序</span>
          <select value={createdOrder} onChange={(event) => setCreatedOrder(event.target.value as AssetCreatedOrder)}>
            <option value="NEWEST">最新优先</option>
            <option value="OLDEST">最早优先</option>
          </select>
        </label>
      </div>

      <div className="filter-row" aria-label="资产分类">
        {categories.map((item) => (
          <button
            key={item.value}
            type="button"
            className={category === item.value ? "filter-button filter-button-active" : "filter-button"}
            onClick={() => setCategory(item.value)}
          >
            {item.label}
          </button>
        ))}
      </div>
      {selectionMode && (
        <section className="asset-bulk-toolbar" aria-label="批量整理素材">
          <strong>已选 {selectedAssetIds.size} 项</strong>
          <button type="button" onClick={() => void runBulkFavorite(true)} disabled={bulkBusy || !selectedAssetIds.size}>收藏</button>
          <button type="button" className="quiet-button" onClick={() => void runBulkFavorite(false)} disabled={bulkBusy || !selectedAssetIds.size}>取消收藏</button>
          <select aria-label="批量操作标签" value={bulkTagId} onChange={(event) => setBulkTagId(event.target.value)} disabled={bulkBusy || !selectedAssetIds.size}>
            <option value="">选择标签</option>
            {tags.map((tag) => <option key={tag.id} value={tag.id}>{tag.name}</option>)}
          </select>
          <button type="button" onClick={() => void runBulkTag(true)} disabled={bulkBusy || !selectedAssetIds.size || !bulkTagId}>添加标签</button>
          <button type="button" className="quiet-button" onClick={() => void runBulkTag(false)} disabled={bulkBusy || !selectedAssetIds.size || !bulkTagId}>移除标签</button>
          <button type="button" className="danger-button" onClick={requestDeleteSelection} disabled={bulkBusy || !selectedAssetIds.size}>删除（{selectedAssetIds.size}）</button>
          <button
            type="button"
            onClick={() => onOpenVideoBatch(selectedVideoAssets)}
            disabled={bulkBusy || selectedVideoAssets.length < 1 || selectedVideoAssets.length > 100}
          >
            批量生成视频（{selectedVideoAssets.length}）
          </button>
          <button type="button" className="quiet-button" onClick={() => setSelectedAssetIds(new Set())} disabled={bulkBusy || !selectedAssetIds.size}>取消选择</button>
        </section>
      )}
      {error && <p className="error-message">资产加载失败：{error}</p>}
      {notice && <p className="studio-notice" role="status">{notice}</p>}
      <AssetGrid
        projectId={projectId}
        assets={assets}
        emptyMessage={emptyMessage}
        onSelect={setSelectedAsset}
        compareMode={compareMode}
        compareIds={compareAssets.map((asset) => asset.id)}
        onToggleCompare={toggleCompare}
        onFavorite={(asset) => void toggleFavorite(asset)}
        selectionMode={selectionMode}
        selectedIds={[...selectedAssetIds]}
        onToggleSelection={toggleBulkSelection}
      />
      {hasFilters && !loading && (
        <button type="button" className="quiet-button clear-asset-filters" onClick={clearFilters}>清除筛选</button>
      )}
      {cursor && (
        <button type="button" className="load-more-button" onClick={() => void requestPage(cursor, false)} disabled={loading}>
          {loading ? "正在加载..." : "加载下一页"}
        </button>
      )}
      {compareMode && (
        <section className="asset-compare-tray" aria-label="对比素材">
          <div>
            <strong>对比素材（{compareAssets.length}/4）</strong>
            <span>选择2到4个相同类型的图片或视频。</span>
          </div>
          <div className="asset-compare-tray-items">
            {compareAssets.map((asset) => (
              <button key={asset.id} type="button" className="quiet-button" onClick={() => toggleCompare(asset)}>
                {asset.name} ×
              </button>
            ))}
            <button type="button" onClick={() => setCompareOpen(true)} disabled={compareAssets.length < 2}>打开对比</button>
            <button type="button" className="quiet-button" onClick={() => setCompareAssets([])} disabled={!compareAssets.length}>清空</button>
          </div>
        </section>
      )}
      {selectedAsset && (
        <AssetPreview
          projectId={projectId}
          asset={selectedAsset}
          onClose={() => setSelectedAsset(undefined)}
          onUseInStudio={onUseInStudio}
          onOpenTask={onOpenTask}
          allTags={tags}
          onOrganizationChanged={applyOrganizationAsset}
          onRequestDelete={(asset) => setDeleteRequest([asset])}
        />
      )}
      {deleteRequest && (
        <AssetDeleteDialog
          projectId={projectId}
          assets={deleteRequest}
          onClose={() => setDeleteRequest(undefined)}
          onDeleted={handleDeleted}
        />
      )}
      {tagManagerOpen && <TagManagerDialog projectId={projectId} onClose={() => setTagManagerOpen(false)} onChanged={(nextTags) => { setTags(nextTags); void requestPage(undefined, true); }} />}
      {compareOpen && compareAssets.length >= 2 && (
        <AssetCompareWorkspace
          projectId={projectId}
          assets={compareAssets}
          onRemove={(assetId) => setCompareAssets((items) => items.filter((asset) => asset.id !== assetId))}
          onClear={() => {
            setCompareAssets([]);
            setCompareOpen(false);
          }}
          onClose={() => setCompareOpen(false)}
        />
      )}
    </section>
  );
}
