import { useEffect, useState } from "react";
import { getAsset, readAssetImage, readAssetThumbnail } from "../../../services/tauriClient";
import type { AssetView } from "../../../types/asset";
import type { DraftValue } from "../../../types/generation";
import { toUserMessage } from "../../../i18n/errorMessages";
import { assetCategoryLabel, formatFileSize } from "../../../i18n/statusLabels";
import { AssetPickerDialog } from "../AssetPickerDialog";

interface Props {
  field: { key: string; label: string; required: boolean };
  value?: DraftValue;
  error?: string;
  projectId: string;
  onChange: (value?: DraftValue) => void;
  onAvailabilityChange?: (available: boolean) => void;
}

export function ImageField({ field, value, error, projectId, onChange, onAvailabilityChange }: Props) {
  const selectedAssetId = value?.type === "image_asset" ? value.assetId : "";
  const [selectedAsset, setSelectedAsset] = useState<AssetView>();
  const [previewUrl, setPreviewUrl] = useState<string>();
  const [pickerOpen, setPickerOpen] = useState(false);
  const [message, setMessage] = useState<string>();

  useEffect(() => {
    setPickerOpen(false);
  }, [projectId]);

  useEffect(() => {
    let active = true;
    setMessage(undefined);
    setSelectedAsset(undefined);
    if (!selectedAssetId) {
      onAvailabilityChange?.(true);
      return () => undefined;
    }
    void getAsset(projectId, selectedAssetId)
      .then((asset) => {
        if (!active) return;
        setSelectedAsset(asset);
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
  }, [onAvailabilityChange, projectId, selectedAssetId]);

  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    setPreviewUrl(undefined);
    if (!selectedAsset) return () => undefined;
    void readAssetThumbnail(projectId, selectedAsset.id)
      .catch(() => readAssetImage(projectId, selectedAsset.id))
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setPreviewUrl(objectUrl);
      })
      .catch((previewError: unknown) => {
        if (active) setMessage(toUserMessage(previewError));
      });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [projectId, selectedAsset]);

  function clearSelection() {
    onChange(undefined);
    setSelectedAsset(undefined);
    setMessage(undefined);
    onAvailabilityChange?.(true);
  }

  return (
    <div className="field-control image-field">
      <span>{field.label}{field.required && <em>必填</em>}</span>
      {selectedAsset ? (
        <div className="asset-field-selection">
          <div className="asset-field-preview image-field-preview">
            {previewUrl ? <img src={previewUrl} alt={selectedAsset.name} /> : <span>正在加载缩略图...</span>}
          </div>
          <div className="asset-field-copy">
            <strong>{selectedAsset.name}</strong>
            <small>{selectedAsset.width ?? "--"} × {selectedAsset.height ?? "--"} · {formatFileSize(selectedAsset.fileSize)}</small>
            <small>{assetCategoryLabel(selectedAsset.category)}</small>
          </div>
          <div className="asset-field-actions">
            <button type="button" onClick={() => setPickerOpen(true)}>更换图片</button>
            <button type="button" className="quiet-button" onClick={clearSelection}>清除</button>
          </div>
        </div>
      ) : (
        <button type="button" className="asset-select-trigger" onClick={() => setPickerOpen(true)}>
          选择参考图片
        </button>
      )}
      {message && <small className="field-error">{message}</small>}
      {error && <small className="field-error">{error}</small>}
      {pickerOpen && (
        <AssetPickerDialog
          projectId={projectId}
          kind="image"
          selectedIds={selectedAssetId ? [selectedAssetId] : []}
          onCancel={() => setPickerOpen(false)}
          onConfirm={(assetIds) => {
            const assetId = assetIds[0];
            if (!assetId) return;
            onChange({ type: "image_asset", assetId });
            setPickerOpen(false);
            setMessage(undefined);
          }}
        />
      )}
    </div>
  );
}
