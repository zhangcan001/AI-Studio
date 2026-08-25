import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  ZoomableImagePreview,
  clampZoomOffset,
  fitScaleFor,
  nextZoomScale,
  resetZoomState,
} from "./ZoomableImagePreview";

describe("ZoomableImagePreview", () => {
  it("shows the compact toolbar for an image and exposes accessible controls", () => {
    const html = renderToStaticMarkup(<ZoomableImagePreview imageUrl="blob:image-1" alt="关键帧" label="关键帧主预览" resetKey="shot-1:image-1" />);
    expect(html).toContain("zoomable-image-toolbar");
    expect(html).toContain("aria-label=\"缩小\"");
    expect(html).toContain("aria-label=\"100% 原始比例\"");
    expect(html).toContain("aria-label=\"放大\"");
    expect(html).toContain("aria-label=\"适合窗口\"");
  });

  it("does not render a toolbar without an image", () => {
    expect(renderToStaticMarkup(<ZoomableImagePreview alt="空预览" />)).toBe("");
  });

  it("keeps fit, 100%, button steps, and pan bounds deterministic", () => {
    expect(fitScaleFor({ width: 800, height: 600 }, { width: 1600, height: 900 })).toBeCloseTo(0.5);
    expect(nextZoomScale(0.8, 1)).toBe(0.9);
    expect(nextZoomScale(0.8, -1)).toBe(0.7);
    expect(resetZoomState(0.5)).toEqual({ mode: "fit", scale: 0.5, offset: { x: 0, y: 0 } });
    expect(clampZoomOffset({ x: 2000, y: -2000 }, 1, { width: 800, height: 600 }, { width: 1600, height: 900 })).toEqual({ x: 424, y: -174 });
  });
});
