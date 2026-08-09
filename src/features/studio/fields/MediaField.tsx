import { useEffect, useMemo, useState } from "react";
import {
  getAsset,
  getAssetMediaUrl,
  listRecentAssets,
  pickAndImportAudio,
  pickAndImportVideo,
  readAssetThumbnail,
} from "../../../services/tauriClient";
import type { AssetView } from "../../../types/asset";
import type { DraftValue } from "../../../types/generation";
import { toUserMessage } from "../../../i18n/errorMessages";
import { assetCategoryLabel, formatDurationMs, formatFileSize } from "../../../i18n/statusLabels";

type MediaKind = "video" | "audio";

interface Props {
  field: {
    key: string;
    label: string;
    required: boolean;
    type: MediaKind;
  };
  value?: DraftValue;
  error?: string;
  projectId: string;
  onChange: (value?: DraftValue) => void;
  onAvailabilityChange?: (available: boolean) => void;
}

export function MediaField({ field, value, error, projectId, onChange, onAvailabilityChange }: Props) {
  const selectedAssetId = selectedId(value, field.type);
  const [recentAssets, setRecentAssets] = useState<AssetView[]>([]);
  const [resolvedAsset, setResolvedAsset] = useState<AssetView>();
  const [posterUrl, setPosterUrl] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string>();

  useEffect(() => {
    let active = true;
    setResolvedAsset(undefined);
    setPosterUrl(undefined);
    setMessage(undefined);
    void listRecentAssets(projectId)
      .then((assets) => {
        if (active) setRecentAssets(assets.filter((asset) => isCompatibleAsset(asset, field.type)));
      })
      .catch((loadError: unknown) => {
        if (active) setMessage(toUserMessage(loadError));
      });
    return () => {
      active = false;
    };
  }, [field.type, projectId]);

  const selectedAsset = useMemo(
    () => recentAssets.find((asset) => asset.id === selectedAssetId) ?? resolvedAsset,
    [recentAssets, resolvedAsset, selectedAssetId],
  );

  useEffect(() => {
    let active = true;
    if (!selectedAssetId) {
      setResolvedAsset(undefined);
      onAvailabilityChange?.(true);
      return () => undefined;
    }
    const recent = recentAssets.find((asset) => asset.id === selectedAssetId);
    if (recent) {
      setResolvedAsset(undefined);
      onAvailabilityChange?.(true);
      return () => undefined;
    }
    void getAsset(projectId, selectedAssetId)
      .then((asset) => {
        if (!active) return;
        if (!isCompatibleAsset(asset, field.type)) {
          throw new Error(`所选素材不是${field.type === "video" ? "视频" : "音频"}。`);
        }
        setResolvedAsset(asset);
        onAvailabilityChange?.(true);
      })
      .catch((loadError: unknown) => {
        if (!active) return;
        setResolvedAsset(undefined);
        setMessage(toUserMessage(loadError));
        onAvailabilityChange?.(false);
      });
    return () => {
      active = false;
    };
  }, [field.type, onAvailabilityChange, projectId, recentAssets, selectedAssetId]);

  useEffect(() => {
    let active = true;
    let nextUrl: string | undefined;
    setPosterUrl(undefined);
    if (field.type !== "video" || !selectedAsset?.thumbnailAvailable) {
      return () => undefined;
    }
    void readAssetThumbnail(projectId, selectedAsset.id)
      .then((bytes) => {
        if (!active) return;
        nextUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setPosterUrl(nextUrl);
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (nextUrl) URL.revokeObjectURL(nextUrl);
    };
  }, [field.type, projectId, selectedAsset]);

  async function chooseLocal() {
    setLoading(true);
    setMessage(undefined);
    try {
      const asset = field.type === "video"
        ? await pickAndImportVideo(projectId)
        : await pickAndImportAudio(projectId);
      if (!asset) return;
      setRecentAssets((current) => [asset, ...current.filter((item) => item.id !== asset.id)]);
      onChange(field.type === "video"
        ? { type: "video_asset", assetId: asset.id }
        : { type: "audio_asset", assetId: asset.id });
      onAvailabilityChange?.(true);
    } catch (pickError: unknown) {
      setMessage(toUserMessage(pickError));
    } finally {
      setLoading(false);
    }
  }

  const mediaUrl = selectedAsset ? getAssetMediaUrl(projectId, selectedAsset.id, field.type) : undefined;

  return (
    <div className="field-control media-field">
      <span>
        {field.label}
        {field.required && <em>必填</em>}
      </span>
      <div className="media-field-actions">
        <button type="button" onClick={() => void chooseLocal()} disabled={loading}>
          {loading ? "正在导入..." : `导入本地${field.type === "video" ? "视频" : "音频"}`}
        </button>
        <select
          aria-label={`${field.label} 最近使用的${field.type === "video" ? "视频" : "音频"}`}
          value={selectedAssetId}
          onChange={(event) => {
            const assetId = event.target.value;
            onChange(assetId ? toDraftValue(field.type, assetId) : undefined);
          }}
        >
          <option value="">选择最近使用的{field.type === "video" ? "视频" : "音频"}</option>
          {recentAssets.map((asset) => (
            <option key={asset.id} value={asset.id}>
              {asset.name} · {assetCategoryLabel(asset.category)}
            </option>
          ))}
        </select>
      </div>
      {selectedAsset && mediaUrl && (
        <div className="media-selection-summary">
          {field.type === "video" ? (
            <video src={mediaUrl} poster={posterUrl} controls preload="metadata" playsInline aria-label={selectedAsset.name} />
          ) : (
            <audio src={mediaUrl} controls preload="metadata" aria-label={selectedAsset.name} />
          )}
          <div>
            <strong>{selectedAsset.name}</strong>
            <small>{formatDurationMs(selectedAsset.durationMs)} · {formatFileSize(selectedAsset.fileSize)}</small>
            {field.type === "video" && <small>{selectedAsset.width ?? "--"} × {selectedAsset.height ?? "--"}</small>}
            <small>{assetCategoryLabel(selectedAsset.category)}</small>
          </div>
        </div>
      )}
      {message && <small className="field-error">{message}</small>}
      {error && <small className="field-error">{error}</small>}
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
  if (kind === "video") {
    return asset.assetType === "video" && ["source_video", "generated_video"].includes(asset.category);
  }
  return asset.assetType === "audio" && asset.category === "source_audio";
}
