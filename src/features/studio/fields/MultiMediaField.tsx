import { useEffect, useMemo, useState } from "react";
import { getAsset, getAssetMediaUrl } from "../../../services/tauriClient";
import type { AssetView } from "../../../types/asset";
import type { DraftValue } from "../../../types/generation";
import { toUserMessage } from "../../../i18n/errorMessages";
import { assetCategoryLabel, formatDurationMs, formatFileSize } from "../../../i18n/statusLabels";
import { AssetPickerDialog } from "../AssetPickerDialog";
import { isCompatibleAsset } from "./MediaField";

type MediaListKind = "videos" | "audios";

interface Props {
  field: { key: string; label: string; required: boolean; minItems: number; maxItems: number; type: MediaListKind };
  value?: DraftValue;
  error?: string;
  projectId: string;
  onChange: (value?: DraftValue) => void;
  onAvailabilityChange?: (available: boolean) => void;
}

export function MultiMediaField({ field, value, error, projectId, onChange, onAvailabilityChange }: Props) {
  const selectedIds = selectedIdsFor(value, field.type);
  const mediaKind = field.type === "videos" ? "video" : "audio";
  const [assetsById, setAssetsById] = useState<Record<string, AssetView>>({});
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
        if (assets.some((asset) => !isCompatibleAsset(asset, mediaKind))) throw new Error("所选素材类型不匹配，请重新选择。");
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
  }, [mediaKind, onAvailabilityChange, projectId, selectedIds.join("\u001f")]);

  const orderedAssets = useMemo(() => selectedIds.map((assetId) => assetsById[assetId]), [assetsById, selectedIds.join("\u001f")]);

  function setIds(assetIds: string[]) {
    onChange(toDraftValue(field.type, assetIds));
    setMessage(undefined);
  }

  return (
    <div className="field-control multi-media-field">
      <span>{field.label}<em>{field.required ? `必填 · ${field.minItems}-${field.maxItems} 个` : `可选 · 最多 ${field.maxItems} 个`}</em></span>
      <button type="button" className="asset-select-trigger" onClick={() => setPickerOpen(true)}>
        {selectedIds.length ? `管理${mediaKind === "video" ? "视频" : "音频"}（${selectedIds.length}/${field.maxItems}）` : `选择${mediaKind === "video" ? "视频" : "音频"}`}
      </button>
      <div className="multi-media-list" aria-label={`${field.label} 已选${mediaKind === "video" ? "视频" : "音频"}`}>
        {orderedAssets.map((asset, index) => asset ? (
          <div key={`${asset.id}-${index}`} className="multi-media-item">
            <span className="multi-media-order" aria-label={`第 ${index + 1} 项`}>{index + 1}</span>
            <div className="multi-media-preview">
              {mediaKind === "video" ? <video src={getAssetMediaUrl(projectId, asset.id, "video")} preload="metadata" muted playsInline aria-label={asset.name} /> : <span className="asset-picker-audio-mark" aria-hidden="true">音频</span>}
            </div>
            <span className="multi-media-name"><strong>{asset.name}</strong><small>{formatDurationMs(asset.durationMs)} · {formatFileSize(asset.fileSize)} · {assetCategoryLabel(asset.category)}</small></span>
            <div className="multi-media-item-actions">
              <button type="button" onClick={() => setIds(selectedIds.filter((_, itemIndex) => itemIndex !== index))}>移除</button>
              <button type="button" onClick={() => index > 0 && setIds(move(selectedIds, index, index - 1))} disabled={index === 0}>上移</button>
              <button type="button" onClick={() => index < selectedIds.length - 1 && setIds(move(selectedIds, index, index + 1))} disabled={index === selectedIds.length - 1}>下移</button>
            </div>
          </div>
        ) : (
          <div className="multi-media-item" key={`${selectedIds[index]}-${index}`}><span>{index + 1}</span><span className="field-error">素材加载失败</span></div>
        ))}
        {!selectedIds.length && <small className="field-hint">按 ComfyUI 接收顺序添加媒体素材。</small>}
      </div>
      {message && <small className="field-error">{message}</small>}
      {error && <small className="field-error">{error}</small>}
      {pickerOpen && (
        <AssetPickerDialog
          projectId={projectId}
          kind={mediaKind}
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

function selectedIdsFor(value: DraftValue | undefined, kind: MediaListKind): string[] {
  if (kind === "videos" && value?.type === "video_assets") return value.assetIds;
  if (kind === "audios" && value?.type === "audio_assets") return value.assetIds;
  return [];
}

function toDraftValue(kind: MediaListKind, assetIds: string[]): DraftValue {
  return kind === "videos" ? { type: "video_assets", assetIds } : { type: "audio_assets", assetIds };
}

export function move(values: string[], from: number, to: number): string[] {
  const next = [...values];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}
