// @vitest-environment jsdom
import { cleanup, render, screen, fireEvent } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { CandidateThumbnail, ThumbnailProjectContext } from "./CreateMediaPreview";

vi.mock("../../product/client", () => ({ productClient: { creation: { mediaUrl: (p: string, id: string, k: string) => `media://${p}/${id}/${k}` } } }));
afterEach(cleanup);
const video = { id: "v1", name: "镜头01", mediaKind: "video" as const, selected: false };

it("uses the backend poster bytes when present", () => {
  URL.createObjectURL = vi.fn(() => "blob:poster"); URL.revokeObjectURL = vi.fn();
  render(<CandidateThumbnail projectId="p" asset={{ ...video, thumbnailBytes: [1, 2] }} />);
  expect(screen.getByRole("img").getAttribute("src")).toBe("blob:poster");
  expect(document.querySelector("video")).toBeNull();
});

it("falls back to the real first frame of the owned video stream, muted and paused", () => {
  render(<ThumbnailProjectContext.Provider value="p"><CandidateThumbnail asset={{ ...video, thumbnailBytes: null }} /></ThumbnailProjectContext.Provider>);
  const node = document.querySelector("video")!;
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
