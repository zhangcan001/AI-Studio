import { useEffect, useMemo, useState } from "react";
import {
  getAsset,
  listRecentAssets,
  pickAndImportImage,
  readAssetImage,
  readAssetThumbnail,
} from "../../../services/tauriClient";
import type { AssetView } from "../../../types/asset";
import type { DraftValue } from "../../../types/generation";
import { toUserMessage } from "../../../i18n/errorMessages";
import { assetCategoryLabel } from "../../../i18n/statusLabels";

interface Props {
  field: {
    key: string;
    label: string;
    required: boolean;
    minItems: number;
    maxItems: number;
  };
  value?: DraftValue;
  error?: string;
  projectId: string;
  onChange: (value?: DraftValue) => void;
  onAvailabilityChange?: (available: boolean) => void;
}

export function MultiImageField({ field, value, error, projectId, onChange, onAvailabilityChange }: Props) {
  const selectedIds = value?.type === "image_assets" ? value.assetIds : [];
  const [recentAssets, setRecentAssets] = useState<AssetView[]>([]);
  const [resolvedAssets, setResolvedAssets] = useState<Record<string, AssetView>>({});
  const [previewUrls, setPreviewUrls] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string>();
  const [pickerValue, setPickerValue] = useState("");

  useEffect(() => {
    let active = true;
    void listRecentAssets(projectId)
      .then((assets) => {
        if (active) setRecentAssets(assets);
      })
      .catch((loadError: unknown) => {
        if (active) setMessage(toUserMessage(loadError));
      });
    return () => {
      active = false;
    };
  }, [projectId]);

  const assetsById = useMemo(() => {
    const map = { ...resolvedAssets };
    for (const asset of recentAssets) map[asset.id] = asset;
    return map;
  }, [recentAssets, resolvedAssets]);

  useEffect(() => {
    let active = true;
    const missingIds = selectedIds.filter((id) => !assetsById[id]);
    if (!missingIds.length) {
      onAvailabilityChange?.(true);
      return () => undefined;
    }
    void Promise.all(missingIds.map((id) => getAsset(projectId, id)))
      .then((assets) => {
        if (!active) return;
        setResolvedAssets((current) => ({
          ...current,
          ...Object.fromEntries(assets.map((asset) => [asset.id, asset])),
        }));
        onAvailabilityChange?.(true);
      })
      .catch(() => {
        if (active) {
          setMessage("找不到一个或多个图片素材，请重新选择。");
          onAvailabilityChange?.(false);
        }
      });
    return () => {
      active = false;
    };
  }, [assetsById, onAvailabilityChange, projectId, selectedIds]);

  useEffect(() => {
    let active = true;
    const created: string[] = [];
    setPreviewUrls({});
    void Promise.all(
      selectedIds.map(async (assetId) => {
        const asset = assetsById[assetId];
        if (!asset) return;
        try {
          const bytes = await readAssetThumbnail(projectId, assetId).catch(() => readAssetImage(projectId, assetId));
          if (!active) return;
          const url = URL.createObjectURL(new Blob([bytes], { type: asset.mimeType }));
          created.push(url);
          setPreviewUrls((current) => ({ ...current, [assetId]: url }));
        } catch {
          // The row remains usable even when the asset binary is unavailable.
        }
      }),
    );
    return () => {
      active = false;
      for (const url of created) URL.revokeObjectURL(url);
    };
  }, [assetsById, projectId, selectedIds]);

  function setIds(assetIds: string[]) {
    onChange({ type: "image_assets", assetIds });
    setMessage(undefined);
  }

  function addAsset(assetId: string) {
    if (!assetId || selectedIds.length >= field.maxItems || selectedIds.includes(assetId)) return;
    setIds([...selectedIds, assetId]);
    setPickerValue("");
  }

  async function chooseLocalImage() {
    if (selectedIds.length >= field.maxItems) return;
    setLoading(true);
    setMessage(undefined);
    try {
      const asset = await pickAndImportImage(projectId);
      if (!asset) return;
      setRecentAssets((current) => [asset, ...current.filter((item) => item.id !== asset.id)]);
      addAsset(asset.id);
    } catch (pickError: unknown) {
      setMessage(toUserMessage(pickError));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="field-control multi-image-field">
      <span>
        {field.label}
        <em>{field.required ? `必填 · ${field.minItems}-${field.maxItems} 张` : `可选 · 最多 ${field.maxItems} 张`}</em>
      </span>
      <div className="multi-image-actions">
        <select
          aria-label={`${field.label} 素材选择器`}
          value={pickerValue}
          onChange={(event) => {
            setPickerValue(event.target.value);
            addAsset(event.target.value);
          }}
          disabled={selectedIds.length >= field.maxItems}
        >
          <option value="">从资产库添加</option>
          {recentAssets.map((asset) => (
            <option key={asset.id} value={asset.id} disabled={selectedIds.includes(asset.id)}>
              {asset.name} · {assetCategoryLabel(asset.category)}
            </option>
          ))}
        </select>
        <button type="button" onClick={() => void chooseLocalImage()} disabled={loading || selectedIds.length >= field.maxItems}>
          {loading ? "正在导入..." : "添加本地图片"}
        </button>
      </div>
      <div className="multi-image-list" aria-label={`${field.label} 已选图片`}>
        {selectedIds.map((assetId, index) => {
          const asset = assetsById[assetId];
          return (
            <div className="multi-image-item" key={`${assetId}-${index}`}>
              <span className="multi-image-order">{index + 1}</span>
              {previewUrls[assetId] && <img src={previewUrls[assetId]} alt={asset?.name ?? assetId} />}
              <span className="multi-image-name">{asset?.name ?? assetId}</span>
              <button type="button" onClick={() => setIds(selectedIds.filter((_, itemIndex) => itemIndex !== index))}>
                移除
              </button>
              <button type="button" onClick={() => index > 0 && setIds(move(selectedIds, index, index - 1))} disabled={index === 0}>
                上移
              </button>
              <button type="button" onClick={() => index < selectedIds.length - 1 && setIds(move(selectedIds, index, index + 1))} disabled={index === selectedIds.length - 1}>
                下移
              </button>
            </div>
          );
        })}
        {!selectedIds.length && <small className="field-hint">请按 ComfyUI 接收顺序添加参考图片。</small>}
      </div>
      {message && <small className="field-error">{message}</small>}
      {error && <small className="field-error">{error}</small>}
    </div>
  );
}

function move(values: string[], from: number, to: number): string[] {
  const next = [...values];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}
