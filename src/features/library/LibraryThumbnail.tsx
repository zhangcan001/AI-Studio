import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { productClient } from "../../product/client";
import type { LibraryItem } from "../../product/libraryTypes";

// One transient IO limiter per mounted Library. It survives page/query changes
// so outstanding old reads and new reads together remain bounded, without cache.
function createThumbnailQueue() {
  let active = 0;
  const pending: Array<() => Promise<void>> = [];
  const pump = () => {
    while (active < 4 && pending.length) {
      const run = pending.shift()!;
      active++;
      void run().finally(() => { active--; pump(); });
    }
  };
  return (run: () => Promise<void>) => {
    pending.push(run); pump();
    return () => { const index = pending.indexOf(run); if (index >= 0) pending.splice(index, 1); };
  };
}
const QueueContext = createContext<ReturnType<typeof createThumbnailQueue> | null>(null);
export function LibraryThumbnailScope({ children }: { children: ReactNode }) {
  const queue = useRef<ReturnType<typeof createThumbnailQueue> | undefined>(undefined);
  if (!queue.current) queue.current = createThumbnailQueue();
  return <QueueContext.Provider value={queue.current}>{children}</QueueContext.Provider>;
}

export function LibraryThumbnail({ projectId, item }: { projectId: string; item: LibraryItem }) {
  const enqueue = useContext(QueueContext);
  const element = useRef<HTMLSpanElement>(null);
  const [url, setUrl] = useState<string>();
  const [failed, setFailed] = useState(false);
  const eligible = item.resourceRef.kind === "asset" && ["image", "video"].includes(item.subtype);
  useEffect(() => {
    setUrl(undefined); setFailed(false);
    if (!eligible || !item.thumbnailAvailable || !element.current || !enqueue || typeof IntersectionObserver === "undefined") return;
    let live = true, requested = false, objectUrl: string | undefined;
    let cancel: (() => void) | undefined;
    const observer = new IntersectionObserver(entries => {
      if (!live || requested || !entries.some(entry => entry.isIntersecting)) return;
      requested = true; observer.disconnect();
      cancel = enqueue(async () => {
        if (!live) return;
        try {
          const bytes = await productClient.library.thumbnailGet(projectId, item.resourceRef);
          if (!live) return;
          // Existing read contract contains bytes only, not authoritative MIME.
          objectUrl = URL.createObjectURL(new Blob([new Uint8Array(bytes)]));
          setUrl(objectUrl);
        } catch { if (live) setFailed(true); }
      });
    }, { root: element.current.closest(".library-list"), rootMargin: "80px", threshold: 0 });
    observer.observe(element.current);
    return () => { live = false; observer.disconnect(); cancel?.(); if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [projectId, item.resourceRef.kind, item.resourceRef.id, item.thumbnailAvailable, eligible, enqueue]);
  if (!eligible) return null;
  return <span ref={element} className="library-thumbnail">{url ? <img src={url} alt={`${item.title}缩略图`} /> : <span>{failed ? "缩略图不可用" : "暂无缩略图"}</span>}</span>;
}
