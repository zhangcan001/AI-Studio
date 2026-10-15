import { createContext, useCallback, useContext, useEffect, useRef, useState, type MouseEvent, type ReactNode } from "react";
import { productClient } from "../../product/client";
import type { CreationAsset } from "../../product/types";

const IMAGE_CACHE_CAP = 32;
type ImageCacheEntry = { promise: Promise<string | undefined>; url?: string; refs: number };
const imageCache = new Map<string, ImageCacheEntry>();
const imageCacheOrder: string[] = [];

function touchImageCache(key: string) {
  const at = imageCacheOrder.indexOf(key);
  if (at >= 0) imageCacheOrder.splice(at, 1);
  imageCacheOrder.push(key);
}

function evictImageCache() {
  while (imageCacheOrder.length > IMAGE_CACHE_CAP) {
    const oldest = imageCacheOrder.shift();
    if (!oldest) break;
    const entry = imageCache.get(oldest);
    if (!entry) continue;
    if (entry.refs > 0) { imageCacheOrder.push(oldest); break; }
    if (entry.url) URL.revokeObjectURL(entry.url);
    imageCache.delete(oldest);
  }
}

/** Shared library imageGet → object URL. LRU-capped; blob URLs revoke on eviction when unreferenced. */
export function loadLibraryImageUrl(projectId: string, assetId: string) {
  const key = `${projectId}:${assetId}`;
  let hit = imageCache.get(key);
  if (!hit) {
    const entry: ImageCacheEntry = {
      refs: 0,
      promise: Promise.resolve()
        .then(() => productClient.library.imageGet(projectId, { kind: "asset", id: assetId }))
        .then(bytes => {
          const url = URL.createObjectURL(new Blob([Uint8Array.from(bytes as number[])], { type: "image/png" }));
          const live = imageCache.get(key);
          if (live) live.url = url;
          return url;
        })
        .catch(() => {
          const live = imageCache.get(key);
          if (live?.url) URL.revokeObjectURL(live.url);
          imageCache.delete(key);
          const at = imageCacheOrder.indexOf(key);
          if (at >= 0) imageCacheOrder.splice(at, 1);
          return undefined;
        }),
    };
    imageCache.set(key, entry);
    touchImageCache(key);
    evictImageCache();
    hit = entry;
  } else {
    touchImageCache(key);
  }
  return hit.promise;
}

export function retainLibraryImage(projectId: string, assetId: string) {
  // Ensure the cache entry exists before taking a ref so eviction cannot drop in-flight loads.
  void loadLibraryImageUrl(projectId, assetId);
  const key = `${projectId}:${assetId}`;
  const entry = imageCache.get(key);
  if (entry) entry.refs += 1;
  return () => {
    const live = imageCache.get(key);
    if (!live) return;
    live.refs = Math.max(0, live.refs - 1);
    if (live.refs === 0) evictImageCache();
  };
}

/** Clears immediately on identity change; ignores late responses for a previous asset. */
export function useLibraryImageUrl(projectId: string | null | undefined, assetId: string | null | undefined) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    setUrl(undefined);
    if (!projectId || !assetId) return;
    let alive = true;
    const release = retainLibraryImage(projectId, assetId);
    void loadLibraryImageUrl(projectId, assetId).then(u => { if (alive) setUrl(u); });
    return () => { alive = false; release(); };
  }, [projectId, assetId]);
  return url;
}

type LightboxTarget =
  | { kind: "video" | "audio"; projectId: string; asset: Pick<CreationAsset, "id" | "name" | "mediaKind"> }
  | { kind: "image"; projectId: string; assetId: string; name: string };

const LightboxContext = createContext<{ open: (t: LightboxTarget) => void } | null>(null);

export function MediaLightboxScope({ children }: { children: ReactNode }) {
  const [target, setTarget] = useState<LightboxTarget>();
  const open = useCallback((t: LightboxTarget) => setTarget(t), []);
  return <LightboxContext.Provider value={{ open }}>
    {children}
    {target && <MediaLightbox target={target} onClose={() => setTarget(undefined)} />}
  </LightboxContext.Provider>;
}

export function useMediaLightbox() {
  return useContext(LightboxContext);
}

/** Full-size preview: video/audio via existing media URL; images via library imageGet. */
function MediaLightbox({ target, onClose }: { target: LightboxTarget; onClose: () => void }) {
  const imageUrl = useLibraryImageUrl(target.kind === "image" ? target.projectId : null, target.kind === "image" ? target.assetId : null);
  const shown = target.kind === "image" ? imageUrl : undefined;
  return <div className="media-lightbox-backdrop" role="presentation" onClick={onClose}>
    <section role="dialog" aria-modal="true" aria-label={target.kind === "image" ? `预览 ${target.name}` : `播放 ${target.asset.name}`} className="media-lightbox" onClick={e => e.stopPropagation()}>
      <header><strong>{target.kind === "image" ? target.name : target.asset.name}</strong><button type="button" onClick={onClose}>关闭</button></header>
      <div className="media-lightbox-body">
        {target.kind === "image"
          ? (shown ? <img src={shown} alt={target.name} /> : <p role="status">正在读取图片…</p>)
          : <CreateMediaPreview projectId={target.projectId} asset={target.asset} />}
      </div>
    </section>
  </div>;
}

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

/** Project scope for candidate thumbnails rendered deep inside list/grid trees. */
export const ThumbnailProjectContext = createContext<string | null>(null);

// Poster first (backend ffmpeg first frame); otherwise the real first frame of the
// owned media stream, muted and paused. Click opens the real playable stream.
export function CandidateThumbnail({ asset, projectId, playable = false }: { asset: CreationAsset; projectId?: string; playable?: boolean }) {
  const scope = useContext(ThumbnailProjectContext);
  const owner = projectId ?? scope;
  const lightbox = useMediaLightbox();
  const [url, setUrl] = useState<string>();
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!asset.thumbnailBytes?.length) { setUrl(undefined); return; }
    const next = URL.createObjectURL(new Blob([Uint8Array.from(asset.thumbnailBytes)], { type: "image/png" }));
    setUrl(next); return () => URL.revokeObjectURL(next);
  }, [asset.thumbnailBytes]);
  useEffect(() => { setFailed(false); }, [owner, asset.id]);
  const open = (e: MouseEvent) => {
    if (!playable || !owner || asset.mediaKind !== "video" || !lightbox) return;
    e.stopPropagation();
    lightbox.open({ kind: "video", projectId: owner, asset: { id: asset.id, name: asset.name, mediaKind: "video" } });
  };
  const frame = url
    ? <img src={url} alt={`${asset.name}缩略图`} />
    : asset.mediaKind === "video" && owner && !failed
      ? <video className="create-candidate-frame" aria-label={`${asset.name}首帧`} src={productClient.creation.mediaUrl(owner, asset.id, "video")} preload="metadata" muted playsInline disablePictureInPicture onError={() => setFailed(true)} />
      : <span className="create-candidate-placeholder">{asset.mediaKind === "video" ? "视频 · 暂无缩略图" : "暂无缩略图"}</span>;
  if (playable && owner && asset.mediaKind === "video" && lightbox) {
    return <button type="button" className="media-thumb-play" aria-label={`播放 ${asset.name}`} title="播放视频" onClick={open}>{frame}<span className="media-thumb-play-badge" aria-hidden="true">▶</span></button>;
  }
  return frame;
}

/** Image reference thumb via library imageGet; click opens enlarged preview (unless interactive=false). */
export function AssetImageThumb({ projectId, assetId, name, size = "md", className, interactive = true }: {
  projectId: string; assetId: string; name: string; size?: "chip" | "md" | "lg"; className?: string; interactive?: boolean;
}) {
  const url = useLibraryImageUrl(projectId, assetId);
  const lightbox = useMediaLightbox();
  const open = (e: MouseEvent) => {
    e.preventDefault(); e.stopPropagation();
    lightbox?.open({ kind: "image", projectId, assetId, name });
  };
  if (!url) return <span className={`asset-image-thumb size-${size} pending ${className ?? ""}`} title={name} aria-label={name}>{name.slice(0, 1)}</span>;
  if (!interactive) return <span className={`asset-image-thumb size-${size} ${className ?? ""}`} title={name}><img src={url} alt={`${name}缩略图`} /></span>;
  return <button type="button" className={`asset-image-thumb size-${size} ${className ?? ""}`} title={`查看 ${name}`} aria-label={`查看 ${name}`} onClick={open}>
    <img src={url} alt={`${name}缩略图`} />
  </button>;
}
