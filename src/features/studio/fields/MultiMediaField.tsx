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
        if (active) setMessage(loadError instanceof Error ? loadError.message : String(loadError));
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
        if (compatible.length !== assets.length) throw new Error(`One or more ${mediaKind} assets are missing.`);
        setResolvedAssets((current) => ({
          ...current,
          ...Object.fromEntries(compatible.map((asset) => [asset.id, asset])),
        }));
        onAvailabilityChange?.(true);
      })
      .catch((loadError: unknown) => {
        if (active) {
          setMessage(loadError instanceof Error ? loadError.message : `Missing ${mediaKind} asset`);
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
      setMessage(pickError instanceof Error ? pickError.message : String(pickError));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="field-control multi-media-field">
      <span>
        {field.label}
        <em>{field.required ? `Required · ${field.minItems}-${field.maxItems}` : `Optional · up to ${field.maxItems}`}</em>
      </span>
      <div className="multi-media-actions">
        <select
          aria-label={`${field.label} asset picker`}
          value={pickerValue}
          onChange={(event) => {
            const assetId = event.target.value;
            setPickerValue(assetId);
            addAsset(assetId);
          }}
          disabled={selectedIds.length >= field.maxItems}
        >
          <option value="">Select a {mediaKind}</option>
          {recentAssets.map((asset) => (
            <option key={asset.id} value={asset.id} disabled={selectedIds.includes(asset.id)}>
              {asset.name} · {asset.category.startsWith("source_") ? "source" : "generated"}
            </option>
          ))}
        </select>
        <button type="button" onClick={() => void chooseLocal()} disabled={loading || selectedIds.length >= field.maxItems}>
          {loading ? "Importing..." : `Import ${mediaKind}`}
        </button>
      </div>
      <div className="multi-media-list" aria-label={`${field.label} selected ${mediaKind}s`}>
        {selectedIds.map((assetId, index) => {
          const asset = assetsById[assetId];
          const url = asset ? getAssetMediaUrl(projectId, asset.id, mediaKind) : undefined;
          return (
            <div key={`${assetId}-${index}`} className="multi-media-item">
              <span className="multi-media-order" aria-label={`Position ${index + 1}`}>{index + 1}</span>
              <div className="multi-media-preview">
                {url && mediaKind === "video" ? (
                  <video src={url} preload="metadata" muted playsInline aria-label={asset?.name ?? "Missing video"} />
                ) : url ? (
                  <audio src={url} preload="metadata" controls aria-label={asset?.name ?? "Missing audio"} />
                ) : <span>Missing Asset</span>}
              </div>
              <span className="multi-media-name">{asset?.name ?? "Missing Asset"}</span>
              <button type="button" onClick={() => setIds(selectedIds.filter((_, itemIndex) => itemIndex !== index))} aria-label={`Remove ${mediaKind} ${index + 1}`}>Remove</button>
              <button type="button" onClick={() => index > 0 && setIds(move(selectedIds, index, index - 1))} disabled={index === 0} aria-label={`Move ${mediaKind} ${index + 1} up`}>Up</button>
              <button type="button" onClick={() => index < selectedIds.length - 1 && setIds(move(selectedIds, index, index + 1))} disabled={index === selectedIds.length - 1} aria-label={`Move ${mediaKind} ${index + 1} down`}>Down</button>
            </div>
          );
        })}
        {!selectedIds.length && <small className="field-hint">Add media in the order ComfyUI should receive it.</small>}
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
