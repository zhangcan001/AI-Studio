import { useEffect, useMemo, useState } from "react";
import { getAsset, readAssetImage, readAssetThumbnail } from "../../../services/tauriClient";
import type { AssetView } from "../../../types/asset";
import type { DraftValue } from "../../../types/generation";
import { toUserMessage } from "../../../i18n/errorMessages";
import { assetCategoryLabel, formatFileSize } from "../../../i18n/statusLabels";
import { AssetPickerDialog } from "../AssetPickerDialog";

interface Props {
  field: { key: string; label: string; required: boolean; minItems: number; maxItems: number };
  value?: DraftValue;
  error?: string;
  projectId: string;
  onChange: (value?: DraftValue) => void;
  onAvailabilityChange?: (available: boolean) => void;
}

export function MultiImageField({ field, value, error, projectId, onChange, onAvailabilityChange }: Props) {
  const selectedIds = value?.type === "image_assets" ? value.assetIds : [];
  const [assetsById, setAssetsById] = useState<Record<string, AssetView>>({});
  const [previewUrls, setPreviewUrls] = useState<Record<string, string>>({});
  const [pickerOpen, setPickerOpen] = useState(false);
  const [message, setMessage] = useState<string>();

  useEffect(() => {
    setPickerOpen(false);
  }, [projectId]);

  useEffect(() => {
    let active = true;
    if (!selectedIds.length) {
      onAvailabilityChange?.(true);
      setAssetsById({});
      return () => undefined;
    }
    void Promise.all(selectedIds.map((assetId) => getAsset(projectId, assetId)))
      .then((assets) => {
        if (!active) return;
        setAssetsById(Object.fromEntries(assets.map((asset) => [asset.id, asset])));
        onAvailabilityChange?.(true);
      })
      .catch((loadError: unknown) => {
        if (!active) return;
        setMessage(toUserMessage(loadError));
        onAvailabilityChange?.(false);
      });
    return () => {
      active = false;
    };
  }, [onAvailabilityChange, projectId, selectedIds.join("\u001f")]);

  useEffect(() => {
    let active = true;
    const created: string[] = [];
    setPreviewUrls({});
    void Promise.all(selectedIds.map(async (assetId) => {
      try {
        const bytes = await readAssetThumbnail(projectId, assetId).catch(() => readAssetImage(projectId, assetId));
        if (!active) return;
        const url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        created.push(url);
        setPreviewUrls((current) => ({ ...current, [assetId]: url }));
      } catch {
        // The ordered row remains editable if a thumbnail is temporarily unavailable.
      }
    }));
    return () => {
      active = false;
      created.forEach((url) => URL.revokeObjectURL(url));
    };
  }, [projectId, selectedIds.join("\u001f")]);

  const orderedAssets = useMemo(() => selectedIds.map((assetId) => assetsById[assetId]), [assetsById, selectedIds.join("\u001f")]);

  function setIds(assetIds: string[]) {
    onChange({ type: "image_assets", assetIds });
    setMessage(undefined);
  }

  return (
    <div className="field-control multi-image-field">
      <span>{field.label}<em>{field.required ? `必填 · ${field.minItems}-${field.maxItems} 张` : `可选 · 最多 ${field.maxItems} 张`}</em></span>
      <button type="button" className="asset-select-trigger" onClick={() => setPickerOpen(true)}>
        {selectedIds.length ? `管理参考图片（${selectedIds.length}/${field.maxItems}）` : "选择参考图片"}
      </button>
      <div className="multi-image-list" aria-label={`${field.label} 已选图片`}>
        {orderedAssets.map((asset, index) => asset ? (
          <div className="multi-image-item" key={`${asset.id}-${index}`}>
            <span className="multi-image-order" aria-label={`第 ${index + 1} 项`}>{index + 1}</span>
            {previewUrls[asset.id] ? <img src={previewUrls[asset.id]} alt={asset.name} /> : <span className="multi-image-placeholder">图片</span>}
            <span className="multi-image-name"><strong>{asset.name}</strong><small>{asset.width ?? "--"} × {asset.height ?? "--"} · {formatFileSize(asset.fileSize)} · {assetCategoryLabel(asset.category)}</small></span>
            <div className="multi-image-item-actions">
              <button type="button" onClick={() => setIds(selectedIds.filter((_, itemIndex) => itemIndex !== index))}>移除</button>
              <button type="button" onClick={() => index > 0 && setIds(move(selectedIds, index, index - 1))} disabled={index === 0}>上移</button>
              <button type="button" onClick={() => index < selectedIds.length - 1 && setIds(move(selectedIds, index, index + 1))} disabled={index === selectedIds.length - 1}>下移</button>
            </div>
          </div>
        ) : (
          <div className="multi-image-item" key={`${selectedIds[index]}-${index}`}><span>{index + 1}</span><span className="field-error">素材加载失败</span></div>
        ))}
        {!selectedIds.length && <small className="field-hint">按 ComfyUI 接收顺序添加参考图片。</small>}
      </div>
      {message && <small className="field-error">{message}</small>}
      {error && <small className="field-error">{error}</small>}
      {pickerOpen && (
        <AssetPickerDialog
          projectId={projectId}
          kind="image"
          multiple
          maxItems={field.maxItems}
          selectedIds={selectedIds}
          onCancel={() => setPickerOpen(false)}
          onConfirm={(assetIds) => {
            setIds(assetIds);
            setPickerOpen(false);
          }}
        />
      )}
    </div>
  );
}

function move(values: string[], from: number, to: number): string[] {
  const next = [...values];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}
