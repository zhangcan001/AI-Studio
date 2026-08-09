import { useEffect, useMemo, useState } from "react";
import {
  getAsset,
  getAssetMediaUrl,
  listRecentAssets,
  pickAndImportAudio,
  pickAndImportVideo,
} from "../../../services/tauriClient";
import type { AssetView } from "../../../types/asset";
import type { DraftValue } from "../../../types/generation";
import { toUserMessage } from "../../../i18n/errorMessages";
import { assetCategoryLabel } from "../../../i18n/statusLabels";
import { isCompatibleAsset } from "./MediaField";

type MediaListKind = "videos" | "audios";

interface Props {
  field: {
    key: string;
    label: string;
    required: boolean;
    minItems: number;
    maxItems: number;
    type: MediaListKind;
  };
  value?: DraftValue;
  error?: string;
  projectId: string;
  onChange: (value?: DraftValue) => void;
  onAvailabilityChange?: (available: boolean) => void;
}

export function MultiMediaField({ field, value, error, projectId, onChange, onAvailabilityChange }: Props) {
  const selectedIds = selectedIdsFor(value, field.type);
  const selectedIdsKey = selectedIds.join("\u001f");
  const mediaKind = field.type === "videos" ? "video" : "audio";
  const [recentAssets, setRecentAssets] = useState<AssetView[]>([]);
  const [resolvedAssets, setResolvedAssets] = useState<Record<string, AssetView>>({});
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string>();
  const [pickerValue, setPickerValue] = useState("");

  useEffect(() => {
    let active = true;
    setMessage(undefined);
    void listRecentAssets(projectId)
      .then((assets) => {
        if (active) setRecentAssets(assets.filter((asset) => isCompatibleAsset(asset, mediaKind)));
      })
      .catch((loadError: unknown) => {
        if (active) setMessage(toUserMessage(loadError));
      });
    return () => {
      active = false;
    };
  }, [mediaKind, projectId]);

  const assetsById = useMemo(() => {
    const next = { ...resolvedAssets };
    for (const asset of recentAssets) next[asset.id] = asset;
    return next;
  }, [recentAssets, resolvedAssets]);

  useEffect(() => {
    let active = true;
    const missing = selectedIds.filter((id) => !assetsById[id]);
    if (!missing.length) {
      onAvailabilityChange?.(true);
      return () => undefined;
    }
    void Promise.all(missing.map((id) => getAsset(projectId, id)))
      .then((assets) => {
        if (!active) return;
        const compatible = assets.filter((asset) => isCompatibleAsset(asset, mediaKind));
        if (compatible.length !== assets.length) throw new Error("找不到一个或多个媒体素材，请重新选择。");
        setResolvedAssets((current) => ({
          ...current,
          ...Object.fromEntries(compatible.map((asset) => [asset.id, asset])),
        }));
        onAvailabilityChange?.(true);
      })
      .catch((loadError: unknown) => {
        if (active) {
          setMessage(toUserMessage(loadError));
          onAvailabilityChange?.(false);
        }
      });
    return () => {
      active = false;
    };
  }, [assetsById, mediaKind, onAvailabilityChange, projectId, selectedIdsKey]);

  function setIds(nextIds: string[]) {
    onChange(toDraftValue(field.type, nextIds));
    setMessage(undefined);
  }

  function addAsset(assetId: string) {
    if (!assetId || selectedIds.length >= field.maxItems) return;
    setIds([...selectedIds, assetId]);
    setPickerValue("");
  }

  async function chooseLocal() {
    if (selectedIds.length >= field.maxItems) return;
    setLoading(true);
    setMessage(undefined);
    try {
      const asset = mediaKind === "video"
        ? await pickAndImportVideo(projectId)
        : await pickAndImportAudio(projectId);
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
    <div className="field-control multi-media-field">
      <span>
        {field.label}
        <em>{field.required ? `必填 · ${field.minItems}-${field.maxItems} 个` : `可选 · 最多 ${field.maxItems} 个`}</em>
      </span>
      <div className="multi-media-actions">
        <select
          aria-label={`${field.label} 素材选择器`}
          value={pickerValue}
          onChange={(event) => {
            const assetId = event.target.value;
            setPickerValue(assetId);
            addAsset(assetId);
          }}
          disabled={selectedIds.length >= field.maxItems}
        >
          <option value="">选择{mediaKind === "video" ? "视频" : "音频"}</option>
          {recentAssets.map((asset) => (
            <option key={asset.id} value={asset.id} disabled={selectedIds.includes(asset.id)}>
              {asset.name} · {assetCategoryLabel(asset.category)}
            </option>
          ))}
        </select>
        <button type="button" onClick={() => void chooseLocal()} disabled={loading || selectedIds.length >= field.maxItems}>
          {loading ? "正在导入..." : `导入${mediaKind === "video" ? "视频" : "音频"}`}
        </button>
      </div>
        <div className="multi-media-list" aria-label={`${field.label} 已选${mediaKind === "video" ? "视频" : "音频"}`}>
        {selectedIds.map((assetId, index) => {
          const asset = assetsById[assetId];
          const url = asset ? getAssetMediaUrl(projectId, asset.id, mediaKind) : undefined;
          return (
            <div key={`${assetId}-${index}`} className="multi-media-item">
              <span className="multi-media-order" aria-label={`第 ${index + 1} 项`}>{index + 1}</span>
              <div className="multi-media-preview">
                {url && mediaKind === "video" ? (
                  <video src={url} preload="metadata" muted playsInline aria-label={asset?.name ?? "缺少视频"} />
                ) : url ? (
                  <audio src={url} preload="metadata" controls aria-label={asset?.name ?? "缺少音频"} />
                ) : <span>缺少素材</span>}
              </div>
              <span className="multi-media-name">{asset?.name ?? "缺少素材"}</span>
              <button type="button" onClick={() => setIds(selectedIds.filter((_, itemIndex) => itemIndex !== index))} aria-label={`移除第 ${index + 1} 项`}>移除</button>
              <button type="button" onClick={() => index > 0 && setIds(move(selectedIds, index, index - 1))} disabled={index === 0} aria-label={`上移第 ${index + 1} 项`}>上移</button>
              <button type="button" onClick={() => index < selectedIds.length - 1 && setIds(move(selectedIds, index, index + 1))} disabled={index === selectedIds.length - 1} aria-label={`下移第 ${index + 1} 项`}>下移</button>
            </div>
          );
        })}
        {!selectedIds.length && <small className="field-hint">请按 ComfyUI 接收顺序添加媒体素材。</small>}
      </div>
      {message && <small className="field-error">{message}</small>}
      {error && <small className="field-error">{error}</small>}
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
