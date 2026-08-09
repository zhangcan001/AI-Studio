import { useEffect, useMemo, useRef, useState } from "react";
import {
  assetLibraryPage,
  listAssetTags,
  pickAndImportAudio,
  pickAndImportImage,
  pickAndImportVideo,
  readAssetThumbnail,
} from "../../services/tauriClient";
import type {
  AssetView,
  PageCursor,
} from "../../types/asset";
import type { AssetTag } from "../../types/organization";
import { toUserMessage } from "../../i18n/errorMessages";
import { assetDisplayName, assetTypeLabel, formatDurationMs, formatFileSize } from "../../i18n/statusLabels";
import {
  buildAssetPickerQuery,
  filterPickerAssets,
  applyAssetPickerAction,
  type AssetPickerFilter,
  type AssetPickerKind,
  toggleAssetSelection,
} from "./assetPicker";

interface Props {
  projectId: string;
  kind: AssetPickerKind;
  multiple?: boolean;
  maxItems?: number;
  selectedIds: string[];
  onCancel: () => void;
  onConfirm: (assetIds: string[]) => void;
}

export function AssetPickerDialog({
  projectId,
  kind,
  multiple = false,
  maxItems = 1,
  selectedIds,
  onCancel,
  onConfirm,
}: Props) {
  const dialogRef = useRef<HTMLElement>(null);
  const [assets, setAssets] = useState<AssetView[]>([]);
  const [selection, setSelection] = useState<string[]>(selectedIds);
  const [filter, setFilter] = useState<AssetPickerFilter>("all");
  const [favoriteOnly, setFavoriteOnly] = useState(false);
  const [tagId, setTagId] = useState("");
  const [tags, setTags] = useState<AssetTag[]>([]);
  const [keywordInput, setKeywordInput] = useState("");
  const [keyword, setKeyword] = useState("");
  const [nextCursor, setNextCursor] = useState<PageCursor>();
  const [loading, setLoading] = useState(true);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string>();
  const requestVersion = useRef(0);

  useEffect(() => {
    const timer = window.setTimeout(() => setKeyword(keywordInput.trim()), 300);
    return () => window.clearTimeout(timer);
  }, [keywordInput]);

  const query = useMemo(() => buildAssetPickerQuery(projectId, kind, filter, keyword, favoriteOnly, tagId), [favoriteOnly, filter, kind, keyword, projectId, tagId]);

  useEffect(() => {
    void listAssetTags(projectId).then(setTags).catch(() => setTags([]));
  }, [projectId]);

  useEffect(() => {
    dialogRef.current?.focus();
    let active = true;
    const version = ++requestVersion.current;
    setLoading(true);
    setError(undefined);
    setAssets([]);
    setNextCursor(undefined);
    void assetLibraryPage(query)
      .then((page) => {
        if (active && requestVersion.current === version) {
          setAssets(page.items);
          setNextCursor(page.nextCursor);
        }
      })
      .catch((loadError: unknown) => {
        if (active && requestVersion.current === version) setError(toUserMessage(loadError));
      })
      .finally(() => {
        if (active && requestVersion.current === version) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [query]);

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") onCancel();
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onCancel]);

  const visibleAssets = useMemo(() => filterPickerAssets(assets, kind, filter), [assets, filter, kind]);
  const title = kind === "image" ? "选择图片" : kind === "video" ? "选择视频" : "选择音频";
  const importLabel = kind === "image" ? "导入本地图片" : kind === "video" ? "导入本地视频" : "导入本地音频";

  async function loadNextPage() {
    if (!nextCursor || loading) return;
    const version = ++requestVersion.current;
    setLoading(true);
    setError(undefined);
    try {
      const page = await assetLibraryPage({ ...query, cursor: nextCursor });
      if (requestVersion.current !== version) return;
      setAssets((current) => {
        const byId = new Map(current.map((asset) => [asset.id, asset]));
        for (const asset of page.items) byId.set(asset.id, asset);
        return [...byId.values()];
      });
      setNextCursor(page.nextCursor);
    } catch (loadError: unknown) {
      if (requestVersion.current === version) setError(toUserMessage(loadError));
    } finally {
      if (requestVersion.current === version) setLoading(false);
    }
  }

  function toggle(assetId: string) {
    setSelection((current) => toggleAssetSelection(current, assetId, multiple, maxItems));
  }

  async function importLocal() {
    setImporting(true);
    setError(undefined);
    try {
      const imported = kind === "image"
        ? await pickAndImportImage(projectId)
        : kind === "video"
          ? await pickAndImportVideo(projectId)
          : await pickAndImportAudio(projectId);
      if (!imported) return;
      setAssets((current) => [imported, ...current.filter((asset) => asset.id !== imported.id)]);
      setSelection((current) => toggleAssetSelection(current, imported.id, multiple, maxItems));
    } catch (importError: unknown) {
      setError(toUserMessage(importError));
    } finally {
      setImporting(false);
    }
  }

  return (
    <div className="asset-picker-backdrop" role="presentation" onMouseDown={onCancel}>
      <section
        ref={dialogRef}
        className="asset-picker-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="asset-picker-title"
        tabIndex={-1}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="asset-picker-heading">
          <div>
            <span className="section-label">素材选择器</span>
            <h2 id="asset-picker-title">{title}</h2>
            <p>{multiple ? `请选择 1 到 ${maxItems} 个素材，顺序会保留。` : "请选择一个当前项目中的素材。"}</p>
          </div>
          <button type="button" className="quiet-button asset-picker-close" onClick={onCancel} aria-label="关闭素材选择器">关闭</button>
        </div>
        <div className="asset-picker-filters" role="tablist" aria-label="素材范围">
          {([
            ["all", "全部"],
            ["source", "源素材"],
            ["generated", "生成素材"],
          ] as const).map(([value, label]) => (
            <button key={value} type="button" role="tab" aria-selected={filter === value} className={filter === value ? "filter-button filter-button-active" : "filter-button"} onClick={() => setFilter(value)}>
              {label}
            </button>
          ))}
        </div>
        <label className="asset-picker-search">
          <span>搜索素材</span>
          <input
            type="search"
            value={keywordInput}
            onChange={(event) => setKeywordInput(event.target.value)}
            placeholder="搜索名称或原始文件名"
            aria-label="搜索素材"
          />
        </label>
        <div className="asset-picker-organization-filters">
          <label className="check-control">
            <input type="checkbox" checked={favoriteOnly} onChange={(event) => setFavoriteOnly(event.target.checked)} />
            <span>仅收藏</span>
          </label>
          <label>
            <span>标签</span>
            <select value={tagId} onChange={(event) => setTagId(event.target.value)}>
              <option value="">全部标签</option>
              {tags.map((tag) => <option key={tag.id} value={tag.id}>{tag.name}</option>)}
            </select>
          </label>
        </div>
        {loading ? (
          <p className="asset-picker-empty" role="status">正在加载当前项目素材...</p>
        ) : visibleAssets.length ? (
          <div className="asset-picker-grid" aria-label="可选素材">
            {visibleAssets.map((asset) => {
              const order = selection.indexOf(asset.id);
              return (
                <PickerAssetCard
                  key={asset.id}
                  projectId={projectId}
                  asset={asset}
                  selected={order >= 0}
                  order={order}
                  onSelect={() => toggle(asset.id)}
                />
              );
            })}
          </div>
        ) : (
          <p className="asset-picker-empty">{keyword ? "没有找到符合条件的素材。" : `当前项目没有可选择的${title.replace("选择", "")}。`}</p>
        )}
        {nextCursor && !loading && (
          <button type="button" className="quiet-button asset-picker-load-more" onClick={() => void loadNextPage()}>
            加载更多
          </button>
        )}
        {error && <p className="error-message" role="alert">{error}</p>}
        <div className="asset-picker-footer">
          <button type="button" className="quiet-button" onClick={() => void importLocal()} disabled={importing}>
            {importing ? "正在导入..." : importLabel}
          </button>
          <div>
            <span className="asset-picker-selection">已选择 {selection.length}{multiple ? ` / ${maxItems}` : ""}</span>
            <button type="button" onClick={() => onConfirm(applyAssetPickerAction(selectedIds, selection, "confirm"))} disabled={!selection.length || importing}>确定</button>
          </div>
        </div>
      </section>
    </div>
  );
}

function PickerAssetCard({
  projectId,
  asset,
  selected,
  order,
  onSelect,
}: {
  projectId: string;
  asset: AssetView;
  selected: boolean;
  order: number;
  onSelect: () => void;
}) {
  const [thumbnailUrl, setThumbnailUrl] = useState<string>();
  const isAudio = asset.assetType === "audio" || asset.category === "source_audio";
  const isVideo = asset.assetType === "video" || asset.category === "source_video" || asset.category === "generated_video";
  const displayName = assetDisplayName(asset);

  useEffect(() => {
    if (isAudio || !asset.thumbnailAvailable) return () => undefined;
    let active = true;
    let objectUrl: string | undefined;
    void readAssetThumbnail(projectId, asset.id)
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setThumbnailUrl(objectUrl);
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [asset.id, asset.thumbnailAvailable, isAudio, projectId]);

  return (
    <button type="button" className={`asset-picker-card${selected ? " asset-picker-card-selected" : ""}`} aria-pressed={selected} onClick={onSelect}>
      <span className="asset-picker-thumb">
        {thumbnailUrl ? <img src={thumbnailUrl} alt={displayName} loading="lazy" /> : isAudio ? <span className="asset-picker-audio-mark" aria-hidden="true">音频</span> : <span className="asset-picker-placeholder">{isVideo ? "视频" : "图片"}</span>}
        {selected && <strong className="asset-picker-order">{order + 1}</strong>}
      </span>
      <span className="asset-picker-card-copy">
        <strong>{displayName}</strong>
        <small>{assetTypeLabel(asset)} · {isVideo || isAudio ? formatDurationMs(asset.durationMs) : `${asset.width ?? "--"} × ${asset.height ?? "--"}`}</small>
        <small>{formatFileSize(asset.fileSize)}</small>
      </span>
    </button>
  );
}
