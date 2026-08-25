import { useCallback, useEffect, useRef, useState } from "react";
import type { CSSProperties, PointerEvent, SyntheticEvent, WheelEvent } from "react";
import "./ZoomableImagePreview.css";

export const MIN_ZOOM_SCALE = 0.2;
export const MAX_ZOOM_SCALE = 4;
export const ZOOM_STEP = 0.1;

const PAN_EDGE_ALLOWANCE = 24;

export interface ZoomImageSize {
  width: number;
  height: number;
}

export interface ZoomOffset {
  x: number;
  y: number;
}

export interface ZoomState {
  mode: "fit" | "custom";
  scale: number;
  offset: ZoomOffset;
}

export function clampZoomScale(value: number): number {
  if (!Number.isFinite(value)) return 1;
  return Math.min(MAX_ZOOM_SCALE, Math.max(MIN_ZOOM_SCALE, value));
}

export function nextZoomScale(current: number, direction: -1 | 1): number {
  const next = current + direction * ZOOM_STEP;
  return clampZoomScale(Math.round(next * 100) / 100);
}

export function fitScaleFor(container: ZoomImageSize, image: ZoomImageSize): number {
  if (container.width <= 0 || container.height <= 0 || image.width <= 0 || image.height <= 0) return 1;
  return clampZoomScale(Math.min(container.width / image.width, container.height / image.height));
}

export function clampZoomOffset(offset: ZoomOffset, scale: number, container: ZoomImageSize, image: ZoomImageSize): ZoomOffset {
  const overflowX = Math.max(0, (image.width * scale - container.width) / 2);
  const overflowY = Math.max(0, (image.height * scale - container.height) / 2);
  const maxX = overflowX > 0 ? overflowX + PAN_EDGE_ALLOWANCE : 0;
  const maxY = overflowY > 0 ? overflowY + PAN_EDGE_ALLOWANCE : 0;
  return {
    x: Math.min(maxX, Math.max(-maxX, offset.x)),
    y: Math.min(maxY, Math.max(-maxY, offset.y)),
  };
}

export function resetZoomState(fitScale = 1): ZoomState {
  return { mode: "fit", scale: clampZoomScale(fitScale), offset: { x: 0, y: 0 } };
}

export function hasZoomPanOverflow(scale: number, container: ZoomImageSize, image: ZoomImageSize): boolean {
  return image.width * scale > container.width + 1 || image.height * scale > container.height + 1;
}

interface ZoomableImagePreviewProps {
  imageUrl?: string;
  alt: string;
  label?: string;
  className?: string;
  resetKey?: string;
}

type ZoomScaleTarget = number | ((current: ZoomState) => number);

export function ZoomableImagePreview({ imageUrl, alt, label, className, resetKey }: ZoomableImagePreviewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ pointerId: number; startX: number; startY: number; origin: ZoomOffset } | null>(null);
  const previousSourceRef = useRef<string | undefined>(undefined);
  const [naturalSize, setNaturalSize] = useState<ZoomImageSize>();
  const [fitScale, setFitScale] = useState(1);
  const [zoom, setZoom] = useState<ZoomState>(() => resetZoomState());
  const [dragging, setDragging] = useState(false);

  const resetPreview = useCallback(() => {
    dragRef.current = null;
    setDragging(false);
    setNaturalSize(undefined);
    setFitScale(1);
    setZoom(resetZoomState());
  }, []);

  const sourceKey = `${resetKey ?? ""}:${imageUrl ?? ""}`;
  useEffect(() => {
    if (previousSourceRef.current === undefined) {
      previousSourceRef.current = sourceKey;
      return;
    }
    if (previousSourceRef.current === sourceKey) return;
    previousSourceRef.current = sourceKey;
    resetPreview();
  }, [resetPreview, sourceKey]);

  const readContainerSize = useCallback((): ZoomImageSize => {
    const container = containerRef.current;
    return { width: container?.clientWidth ?? 0, height: container?.clientHeight ?? 0 };
  }, []);

  const updateFitScale = useCallback((imageSize = naturalSize) => {
    if (!imageSize) return;
    const nextFitScale = fitScaleFor(readContainerSize(), imageSize);
    setFitScale(nextFitScale);
    setZoom((current) => current.mode === "fit"
      ? resetZoomState(nextFitScale)
      : { ...current, offset: clampZoomOffset(current.offset, current.scale, readContainerSize(), imageSize) });
  }, [naturalSize, readContainerSize]);

  useEffect(() => {
    if (!naturalSize || !containerRef.current) return undefined;
    const update = () => updateFitScale(naturalSize);
    update();
    let observer: ResizeObserver | undefined;
    if (typeof ResizeObserver !== "undefined") {
      observer = new ResizeObserver(update);
      observer.observe(containerRef.current);
    }
    window.addEventListener("resize", update);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", update);
    };
  }, [naturalSize, updateFitScale]);

  const handleImageLoad = useCallback((event: SyntheticEvent<HTMLImageElement>) => {
    const image = event.currentTarget;
    if (!image.naturalWidth || !image.naturalHeight) return;
    const nextSize = { width: image.naturalWidth, height: image.naturalHeight };
    setNaturalSize(nextSize);
    const nextFitScale = fitScaleFor(readContainerSize(), nextSize);
    setFitScale(nextFitScale);
    setZoom(resetZoomState(nextFitScale));
  }, [readContainerSize]);

  const applyScale = useCallback((target: ZoomScaleTarget, focalPoint?: { x: number; y: number }) => {
    setZoom((current) => {
      const nextScale = clampZoomScale(typeof target === "function" ? target(current) : target);
      let nextOffset = current.offset;
      const imageSize = naturalSize;
      const container = containerRef.current;
      if (focalPoint && imageSize && container && current.scale > 0) {
        const bounds = container.getBoundingClientRect();
        const focusX = focalPoint.x - (bounds.left + bounds.width / 2) - current.offset.x;
        const focusY = focalPoint.y - (bounds.top + bounds.height / 2) - current.offset.y;
        const ratio = nextScale / current.scale;
        nextOffset = {
          x: current.offset.x - focusX * (ratio - 1),
          y: current.offset.y - focusY * (ratio - 1),
        };
      }
      if (imageSize) nextOffset = clampZoomOffset(nextOffset, nextScale, readContainerSize(), imageSize);
      return { mode: "custom", scale: nextScale, offset: nextOffset };
    });
  }, [naturalSize, readContainerSize]);

  const fitToView = useCallback(() => {
    setZoom(resetZoomState(fitScale));
  }, [fitScale]);

  const handleWheel = useCallback((event: WheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    applyScale((current) => nextZoomScale(current.scale, event.deltaY > 0 ? -1 : 1), { x: event.clientX, y: event.clientY });
  }, [applyScale]);

  const handlePointerDown = useCallback((event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !naturalSize) return;
    if (!hasZoomPanOverflow(zoom.scale, readContainerSize(), naturalSize)) return;
    if ((event.target as HTMLElement).closest?.("button")) return;
    dragRef.current = { pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, origin: zoom.offset };
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragging(true);
  }, [naturalSize, readContainerSize, zoom.offset, zoom.scale]);

  const handlePointerMove = useCallback((event: PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId || !naturalSize) return;
    setZoom((current) => {
      const nextOffset = clampZoomOffset({
        x: drag.origin.x + event.clientX - drag.startX,
        y: drag.origin.y + event.clientY - drag.startY,
      }, current.scale, readContainerSize(), naturalSize);
      return { ...current, offset: nextOffset };
    });
  }, [naturalSize, readContainerSize]);

  const finishPointerDrag = useCallback((event: PointerEvent<HTMLDivElement>) => {
    if (dragRef.current?.pointerId !== event.pointerId) return;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    dragRef.current = null;
    setDragging(false);
  }, []);

  const handleDoubleClick = useCallback(() => {
    setZoom((current) => current.mode === "fit" ? { ...resetZoomState(1), mode: "custom" } : resetZoomState(fitScale));
  }, [fitScale]);

  if (!imageUrl) return null;

  const containerSize = readContainerSize();
  const panEnabled = Boolean(naturalSize && hasZoomPanOverflow(zoom.scale, containerSize, naturalSize));
  const imageStyle: CSSProperties = {
    left: `calc(50% + ${zoom.offset.x}px)`,
    top: `calc(50% + ${zoom.offset.y}px)`,
    transform: `translate3d(-50%, -50%, 0) scale(${zoom.scale})`,
    ...(naturalSize ? { width: naturalSize.width, height: naturalSize.height } : {}),
  };

  return (
    <div
      ref={containerRef}
      className={`zoomable-image-preview${className ? ` ${className}` : ""}${dragging ? " zoomable-image-preview-dragging" : ""}`}
      role="group"
      aria-label={label ?? alt}
      data-pan-enabled={panEnabled ? "true" : "false"}
      onWheel={handleWheel}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={finishPointerDrag}
      onPointerCancel={finishPointerDrag}
      onDoubleClick={handleDoubleClick}
    >
      <img className="zoomable-image-preview-image" src={imageUrl} alt={alt} draggable={false} style={imageStyle} onLoad={handleImageLoad} />
      <div className="zoomable-image-toolbar" aria-label="预览缩放工具" onPointerDown={(event) => event.stopPropagation()}>
        <button type="button" aria-label="缩小" title="缩小" onClick={() => applyScale((current) => nextZoomScale(current.scale, -1))} disabled={zoom.scale <= MIN_ZOOM_SCALE}>−</button>
        <button type="button" aria-label="100% 原始比例" title="100% 原始比例" onClick={() => applyScale(1)}>100%</button>
        <button type="button" aria-label="放大" title="放大" onClick={() => applyScale((current) => nextZoomScale(current.scale, 1))} disabled={zoom.scale >= MAX_ZOOM_SCALE}>＋</button>
        <button type="button" aria-label="适合窗口" title="适合窗口" onClick={fitToView}>适合</button>
        <span className="zoomable-image-percent" aria-live="polite">{Math.round(zoom.scale * 100)}%</span>
      </div>
    </div>
  );
}
