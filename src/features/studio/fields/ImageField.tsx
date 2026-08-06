import { useEffect, useMemo, useState } from "react";
import {
  listRecentAssets,
  pickAndImportImage,
  readAssetImage,
} from "../../../services/tauriClient";
import type { AssetView } from "../../../types/asset";
import type { DraftValue } from "../../../types/generation";

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
}

export function ImageField({ field, value, error, projectId, onChange }: Props) {
  const selectedAssetId = value?.type === "image_asset" ? value.assetId : "";
  const [recentAssets, setRecentAssets] = useState<AssetView[]>([]);
  const [previewUrl, setPreviewUrl] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string>();

  useEffect(() => {
    let active = true;
    void listRecentAssets(projectId)
      .then((assets) => {
        if (active) setRecentAssets(assets);
      })
      .catch((loadError: unknown) => {
        if (active) setMessage(loadError instanceof Error ? loadError.message : String(loadError));
      });
    return () => {
      active = false;
    };
  }, [projectId]);

  const selectedAsset = useMemo(
    () => recentAssets.find((asset) => asset.id === selectedAssetId),
    [recentAssets, selectedAssetId],
  );

  useEffect(() => {
    let active = true;
    let nextUrl: string | undefined;
    if (!selectedAsset) {
      setPreviewUrl(undefined);
      return () => undefined;
    }
    void readAssetImage(selectedAsset.id)
      .then((bytes) => {
        if (!active) return;
        nextUrl = URL.createObjectURL(new Blob([bytes], { type: selectedAsset.mimeType }));
        setPreviewUrl(nextUrl);
      })
      .catch((previewError: unknown) => {
        if (active) setMessage(previewError instanceof Error ? previewError.message : String(previewError));
      });
    return () => {
      active = false;
      if (nextUrl) URL.revokeObjectURL(nextUrl);
    };
  }, [selectedAsset]);

  async function chooseLocalImage() {
    setLoading(true);
    setMessage(undefined);
    try {
      const asset = await pickAndImportImage(projectId);
      if (!asset) return;
      setRecentAssets((current) => [asset, ...current.filter((item) => item.id !== asset.id)]);
      onChange({ type: "image_asset", assetId: asset.id });
    } catch (pickError: unknown) {
      setMessage(pickError instanceof Error ? pickError.message : String(pickError));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="field-control image-field">
      <span>
        {field.label}
        {field.required && <em>Required</em>}
      </span>
      <div className="image-field-actions">
        <button type="button" onClick={() => void chooseLocalImage()} disabled={loading}>
          {loading ? "Importing..." : "Choose local image"}
        </button>
        <select
          aria-label={`${field.label} recent images`}
          value={selectedAssetId}
          onChange={(event) => {
            const assetId = event.target.value;
            onChange(assetId ? { type: "image_asset", assetId } : undefined);
          }}
        >
          <option value="">Select a recent image</option>
          {recentAssets.map((asset) => (
            <option key={asset.id} value={asset.id}>
              {asset.name} · {asset.category === "source_image" ? "source" : "generated"}
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
              {selectedAsset.width} × {selectedAsset.height} · {(selectedAsset.fileSize / 1024).toFixed(1)} KB
            </small>
            <small>{selectedAsset.category === "source_image" ? "Source image" : "Generated image"}</small>
          </div>
        </div>
      )}
      {message && <small className="field-error">{message}</small>}
      {error && <small className="field-error">{error}</small>}
    </div>
  );
}
