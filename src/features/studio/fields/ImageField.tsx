import { useEffect, useMemo, useState } from "react";
import {
  getAsset,
  listRecentAssets,
  pickAndImportImage,
  readAssetThumbnail,
  readAssetImage,
} from "../../../services/tauriClient";
import type { AssetView } from "../../../types/asset";
import type { DraftValue } from "../../../types/generation";
import { toUserMessage } from "../../../i18n/errorMessages";
import { assetCategoryLabel, formatFileSize } from "../../../i18n/statusLabels";

interface Props {
  field: {
    key: string;
    label: string;
    required: boolean;
  };
  value?: DraftValue;
  error?: string;
  projectId: string;
  onChange: (value?: DraftValue) => void;
  onAvailabilityChange?: (available: boolean) => void;
}

export function ImageField({ field, value, error, projectId, onChange, onAvailabilityChange }: Props) {
  const selectedAssetId = value?.type === "image_asset" ? value.assetId : "";
  const [recentAssets, setRecentAssets] = useState<AssetView[]>([]);
  const [previewUrl, setPreviewUrl] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string>();
  const [resolvedAsset, setResolvedAsset] = useState<AssetView>();

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

  const recentSelectedAsset = useMemo(
    () => recentAssets.find((asset) => asset.id === selectedAssetId),
    [recentAssets, selectedAssetId],
  );

  const selectedAsset = recentSelectedAsset ?? resolvedAsset;

  useEffect(() => {
    let active = true;
    setMessage(undefined);
    if (!selectedAssetId) {
      setResolvedAsset(undefined);
      onAvailabilityChange?.(true);
      return () => undefined;
    }
    if (recentSelectedAsset) {
      setResolvedAsset(undefined);
      onAvailabilityChange?.(true);
      return () => undefined;
    }
    void getAsset(projectId, selectedAssetId)
      .then((asset) => {
        if (!active) return;
        setResolvedAsset(asset);
        onAvailabilityChange?.(true);
      })
      .catch(() => {
        if (!active) return;
        setResolvedAsset(undefined);
        setMessage("找不到图片素材，请重新选择。");
        onAvailabilityChange?.(false);
      });
    return () => {
      active = false;
    };
  }, [onAvailabilityChange, projectId, recentSelectedAsset, selectedAssetId]);

  useEffect(() => {
    let active = true;
    let nextUrl: string | undefined;
    if (!selectedAsset) {
      setPreviewUrl(undefined);
      return () => undefined;
    }
    void readAssetThumbnail(projectId, selectedAsset.id)
      .catch(() => readAssetImage(projectId, selectedAsset.id))
      .then((bytes) => {
        if (!active) return;
        nextUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setPreviewUrl(nextUrl);
      })
      .catch((previewError: unknown) => {
        if (active) {
          setMessage(toUserMessage(previewError));
          onAvailabilityChange?.(false);
        }
      });
    return () => {
      active = false;
      if (nextUrl) URL.revokeObjectURL(nextUrl);
    };
  }, [onAvailabilityChange, projectId, selectedAsset]);

  async function chooseLocalImage() {
    setLoading(true);
    setMessage(undefined);
    try {
      const asset = await pickAndImportImage(projectId);
      if (!asset) return;
      setRecentAssets((current) => [asset, ...current.filter((item) => item.id !== asset.id)]);
      onChange({ type: "image_asset", assetId: asset.id });
      onAvailabilityChange?.(true);
    } catch (pickError: unknown) {
      setMessage(toUserMessage(pickError));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="field-control image-field">
      <span>
        {field.label}
        {field.required && <em>必填</em>}
      </span>
      <div className="image-field-actions">
        <button type="button" onClick={() => void chooseLocalImage()} disabled={loading}>
          {loading ? "正在导入..." : "选择本地图片"}
        </button>
        <select
          aria-label={`${field.label} 最近使用的图片`}
          value={selectedAssetId}
          onChange={(event) => {
            const assetId = event.target.value;
            onChange(assetId ? { type: "image_asset", assetId } : undefined);
          }}
        >
          <option value="">选择最近使用的图片</option>
          {recentAssets.map((asset) => (
            <option key={asset.id} value={asset.id}>
              {asset.name} · {assetCategoryLabel(asset.category)}
            </option>
          ))}
        </select>
      </div>
      {selectedAsset && (
        <div className="image-selection-summary">
          {previewUrl && <img src={previewUrl} alt={selectedAsset.name} />}
          <div>
            <strong>{selectedAsset.name}</strong>
            <small>
              {selectedAsset.width} × {selectedAsset.height} · {formatFileSize(selectedAsset.fileSize)}
            </small>
            <small>{assetCategoryLabel(selectedAsset.category)}</small>
          </div>
        </div>
      )}
      {message && <small className="field-error">{message}</small>}
      {error && <small className="field-error">{error}</small>}
    </div>
  );
}
