import { useEffect, useState } from "react";
import { getAsset, getAssetMediaUrl, readAssetThumbnail } from "../../../services/tauriClient";
import type { AssetView } from "../../../types/asset";
import type { DraftValue } from "../../../types/generation";
import { toUserMessage } from "../../../i18n/errorMessages";
import { assetCategoryLabel, formatDurationMs, formatFileSize } from "../../../i18n/statusLabels";
import { AssetPickerDialog } from "../AssetPickerDialog";

export type MediaKind = "video" | "audio";

interface Props {
  field: { key: string; label: string; required: boolean; type: MediaKind };
  value?: DraftValue;
  error?: string;
  projectId: string;
  onChange: (value?: DraftValue) => void;
  onAvailabilityChange?: (available: boolean) => void;
}

export function MediaField({ field, value, error, projectId, onChange, onAvailabilityChange }: Props) {
  const selectedAssetId = selectedId(value, field.type);
  const [selectedAsset, setSelectedAsset] = useState<AssetView>();
  const [posterUrl, setPosterUrl] = useState<string>();
  const [pickerOpen, setPickerOpen] = useState(false);
  const [message, setMessage] = useState<string>();

  useEffect(() => {
    setPickerOpen(false);
  }, [projectId]);

  useEffect(() => {
    let active = true;
    setSelectedAsset(undefined);
    setMessage(undefined);
    if (!selectedAssetId) {
      onAvailabilityChange?.(true);
      return () => undefined;
    }
    void getAsset(projectId, selectedAssetId)
      .then((asset) => {
        if (!active) return;
        if (!isCompatibleAsset(asset, field.type)) throw new Error(`所选素材不是${field.type === "video" ? "视频" : "音频"}。`);
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
  }, [field.type, onAvailabilityChange, projectId, selectedAssetId]);

  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    setPosterUrl(undefined);
    if (field.type !== "video" || !selectedAsset?.thumbnailAvailable) return () => undefined;
    void readAssetThumbnail(projectId, selectedAsset.id)
      .then((bytes) => {
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setPosterUrl(objectUrl);
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [field.type, projectId, selectedAsset]);

  const mediaUrl = selectedAsset ? getAssetMediaUrl(projectId, selectedAsset.id, field.type) : undefined;

  function clearSelection() {
    onChange(undefined);
    setSelectedAsset(undefined);
    onAvailabilityChange?.(true);
  }

  return (
    <div className="field-control media-field">
      <span>{field.label}<em>{field.required ? "必填" : "可选"}</em></span>
      {selectedAsset && mediaUrl ? (
        <div className="asset-field-selection">
          <div className="asset-field-preview media-field-preview">
            {field.type === "video" ? <video src={mediaUrl} poster={posterUrl} controls preload="metadata" playsInline aria-label={selectedAsset.name} /> : <div className="audio-summary"><span className="asset-picker-audio-mark" aria-hidden="true">音频</span><audio src={mediaUrl} controls preload="metadata" aria-label={selectedAsset.name} /></div>}
          </div>
          <div className="asset-field-copy">
            <strong>{selectedAsset.name}</strong>
            <small>{field.type === "video" ? `${selectedAsset.width ?? "--"} × ${selectedAsset.height ?? "--"} · ` : ""}{formatDurationMs(selectedAsset.durationMs)} · {formatFileSize(selectedAsset.fileSize)}</small>
            <small>{assetCategoryLabel(selectedAsset.category)}</small>
          </div>
          <div className="asset-field-actions">
            <button type="button" onClick={() => setPickerOpen(true)}>更换{field.type === "video" ? "视频" : "音频"}</button>
            <button type="button" className="quiet-button" onClick={clearSelection}>清除</button>
          </div>
        </div>
      ) : (
        <button type="button" className="asset-select-trigger" onClick={() => setPickerOpen(true)}>
          选择{field.type === "video" ? "参考视频" : "参考音频"}
        </button>
      )}
      {message && <small className="field-error">{message}</small>}
      {error && <small className="field-error">{error}</small>}
      {pickerOpen && (
        <AssetPickerDialog
          projectId={projectId}
          kind={field.type}
          selectedIds={selectedAssetId ? [selectedAssetId] : []}
          onCancel={() => setPickerOpen(false)}
          onConfirm={(assetIds) => {
            const assetId = assetIds[0];
            if (!assetId) return;
            onChange(toDraftValue(field.type, assetId));
            setPickerOpen(false);
            setMessage(undefined);
          }}
        />
      )}
    </div>
  );
}

function selectedId(value: DraftValue | undefined, kind: MediaKind): string {
  if (kind === "video" && value?.type === "video_asset") return value.assetId;
  if (kind === "audio" && value?.type === "audio_asset") return value.assetId;
  return "";
}

function toDraftValue(kind: MediaKind, assetId: string): DraftValue {
  return kind === "video" ? { type: "video_asset", assetId } : { type: "audio_asset", assetId };
}

export function isCompatibleAsset(asset: AssetView, kind: MediaKind): boolean {
  if (kind === "video") return asset.assetType === "video" && ["source_video", "generated_video"].includes(asset.category);
  return asset.assetType === "audio" && asset.category === "source_audio";
}
