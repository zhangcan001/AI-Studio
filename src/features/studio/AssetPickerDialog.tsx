import { useEffect, useMemo, useRef, useState } from "react";
import {
  listRecentAssets,
  pickAndImportAudio,
  pickAndImportImage,
  pickAndImportVideo,
  readAssetThumbnail,
} from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import { toUserMessage } from "../../i18n/errorMessages";
import { assetDisplayName, assetTypeLabel, formatDurationMs, formatFileSize } from "../../i18n/statusLabels";
import {
  filterPickerAssets,
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
  const [loading, setLoading] = useState(true);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    dialogRef.current?.focus();
    let active = true;
    setLoading(true);
    void listRecentAssets(projectId, 100)
      .then((nextAssets) => {
        if (active) setAssets(nextAssets);
      })
      .catch((loadError: unknown) => {
        if (active) setError(toUserMessage(loadError));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [projectId]);

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
          <p className="asset-picker-empty">当前项目没有可选择的{title.replace("选择", "")}。</p>
        )}
        {error && <p className="error-message" role="alert">{error}</p>}
        <div className="asset-picker-footer">
          <button type="button" className="quiet-button" onClick={() => void importLocal()} disabled={importing}>
            {importing ? "正在导入..." : importLabel}
          </button>
          <div>
            <span className="asset-picker-selection">已选择 {selection.length}{multiple ? ` / ${maxItems}` : ""}</span>
            <button type="button" onClick={() => onConfirm(selection)} disabled={!selection.length || importing}>确定</button>
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
