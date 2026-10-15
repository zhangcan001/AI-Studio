// @vitest-environment jsdom
import { cleanup, render, screen, fireEvent } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AssetImageThumb, CandidateThumbnail, MediaLightboxScope, ThumbnailProjectContext } from "./CreateMediaPreview";

const imageGet = vi.fn();
vi.mock("../../product/client", () => ({
  productClient: {
    creation: { mediaUrl: (p: string, id: string, k: string) => `media://${p}/${id}/${k}` },
    library: { imageGet: (...args: unknown[]) => imageGet(...args) },
  },
}));
afterEach(() => { cleanup(); imageGet.mockReset(); });
const video = { id: "v1", name: "镜头01", mediaKind: "video" as const, selected: false };

it("uses the backend poster bytes when present", () => {
  URL.createObjectURL = vi.fn(() => "blob:poster"); URL.revokeObjectURL = vi.fn();
  render(<MediaLightboxScope><CandidateThumbnail projectId="p" asset={{ ...video, thumbnailBytes: [1, 2] }} /></MediaLightboxScope>);
  expect(screen.getByRole("img").getAttribute("src")).toBe("blob:poster");
  expect(document.querySelector("video.create-candidate-frame")).toBeNull();
});

it("falls back to the real first frame of the owned video stream, muted and paused", () => {
  render(<MediaLightboxScope><ThumbnailProjectContext.Provider value="p"><CandidateThumbnail asset={{ ...video, thumbnailBytes: null }} /></ThumbnailProjectContext.Provider></MediaLightboxScope>);
  const node = document.querySelector("video.create-candidate-frame") as HTMLVideoElement;
  expect(node.getAttribute("src")).toBe("media://p/v1/video");
  expect(node.getAttribute("preload")).toBe("metadata");
  expect(node.muted).toBe(true); expect(node.autoplay).toBe(false); expect(node.controls).toBe(false);
  fireEvent.error(node);
  expect(screen.getByText("视频 · 暂无缩略图")).toBeTruthy();
});

it("shows an honest placeholder without a project scope", () => {
  render(<CandidateThumbnail asset={video} />);
  expect(document.querySelector("video")).toBeNull();
  expect(screen.getByText("视频 · 暂无缩略图")).toBeTruthy();
});

it("opens a playable lightbox with controls when the video thumb is clicked", () => {
  render(<MediaLightboxScope><CandidateThumbnail projectId="p" asset={{ ...video, thumbnailBytes: [1, 2] }} /></MediaLightboxScope>);
  URL.createObjectURL = vi.fn(() => "blob:poster");
  fireEvent.click(screen.getByRole("button", { name: "播放 镜头01" }));
  const dialog = screen.getByRole("dialog", { name: "播放 镜头01" });
  expect(dialog.querySelector("video[controls]")).toBeTruthy();
  expect(dialog.querySelector("video[controls]")!.getAttribute("src")).toBe("media://p/v1/video");
});

it("loads reference image thumbs through library imageGet and opens an enlarge preview", async () => {
  imageGet.mockResolvedValue([9, 9, 9]);
  URL.createObjectURL = vi.fn(() => "blob:img"); URL.revokeObjectURL = vi.fn();
  render(<MediaLightboxScope><AssetImageThumb projectId="p" assetId="img1" name="首帧" size="lg" /></MediaLightboxScope>);
  const btn = await screen.findByRole("button", { name: "查看 首帧" });
  expect(imageGet).toHaveBeenCalledWith("p", { kind: "asset", id: "img1" });
  expect(btn.querySelector("img")!.getAttribute("src")).toBe("blob:img");
  fireEvent.click(btn);
  expect(screen.getByRole("dialog", { name: "预览 首帧" }).querySelector("img")!.getAttribute("src")).toBe("blob:img");
});
