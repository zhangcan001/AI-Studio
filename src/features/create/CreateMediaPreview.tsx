import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import type { CreationAsset } from "../../product/types";

// Presentation only: explicit project asset identities, never paths or global scans.
export function CreateMediaPreview({ projectId, asset, onUnavailable }: {
  projectId: string; asset: Pick<CreationAsset, "id" | "name" | "mediaKind">;
  onUnavailable?: () => void;
}) {
  const [image, setImage] = useState<string>();
  const [failed, setFailed] = useState(false);
  const media = useRef<HTMLVideoElement | HTMLAudioElement>(null);
  useEffect(() => {
    let alive = true; let url: string | undefined;
    const player = media.current;
    setImage(undefined); setFailed(false);
    // StrictMode replays setup after cleanup on the same DOM node. Restore the
    // controlled URL we detached, not a filesystem path or a new media authority.
    if (player && asset.mediaKind !== "image" && !player.getAttribute("src")) {
      player.src = productClient.creation.mediaUrl(projectId, asset.id, asset.mediaKind);
      player.load();
    }
    if (asset.mediaKind === "image") void Promise.resolve().then(async () => {
      const detail = await productClient.library.get(projectId, { kind: "asset", id: asset.id });
      if (!alive) return;
      if (detail.kind !== "asset" || detail.asset.assetType !== "image") throw Error("Unavailable image");
      const bytes = await productClient.library.imageGet(projectId, { kind: "asset", id: asset.id });
      if (!alive) return;
      url = URL.createObjectURL(new Blob([Uint8Array.from(bytes)], { type: detail.asset.mimeType }));
      setImage(url);
    }).catch(() => { if (alive) { setFailed(true); onUnavailable?.(); } });
    return () => {
      alive = false;
      if (url) URL.revokeObjectURL(url);
      // Detach the controlled stream on candidate/owner change, even if paused.
      if (player) { player.pause(); player.removeAttribute("src"); player.load(); }
    };
  }, [projectId, asset.id, asset.mediaKind]);
  const unavailable = () => { setFailed(true); onUnavailable?.(); };
  if (failed) return <p role="status">媒体不可用或无法预览，请刷新或在资源库检查；历史关系仍保留。</p>;
  if (asset.mediaKind === "image") return image ? <img src={image} alt={asset.name} onError={unavailable} /> : <p role="status">正在读取图片…</p>;
  const src = productClient.creation.mediaUrl(projectId, asset.id, asset.mediaKind);
  return asset.mediaKind === "video"
    ? <video ref={media as React.RefObject<HTMLVideoElement>} aria-label={asset.name} controls preload="metadata" src={src} onError={unavailable} />
    : <audio ref={media as React.RefObject<HTMLAudioElement>} aria-label={asset.name} controls preload="metadata" src={src} onError={unavailable} />;
}

export function CandidateThumbnail({ asset }: { asset: CreationAsset }) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    if (!asset.thumbnailBytes?.length) { setUrl(undefined); return; }
    const next = URL.createObjectURL(new Blob([Uint8Array.from(asset.thumbnailBytes)], { type: "image/png" }));
    setUrl(next); return () => URL.revokeObjectURL(next);
  }, [asset.thumbnailBytes]);
  return url ? <img src={url} alt={`${asset.name}缩略图`} /> : <span className="create-candidate-placeholder">视频 · 暂无缩略图</span>;
}
