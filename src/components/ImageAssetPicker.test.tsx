// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ImageAssetPicker } from "./ImageAssetPicker";
import { readFileSync } from "node:fs";

const api = vi.hoisted(() => ({
  list: vi.fn(),
  tagsList: vi.fn(),
  imageGet: vi.fn(),
}));
vi.mock("../product/client", () => ({ productClient: { library: api } }));

beforeEach(() => {
  vi.resetAllMocks();
  (URL as unknown as { createObjectURL: () => string }).createObjectURL = () => "blob:img";
  api.tagsList.mockResolvedValue([{ id: "t-char", name: "角色" }]);
  api.list.mockResolvedValue({
    items: [
      { resourceRef: { kind: "asset", id: "a1" }, title: "角色甲", subtype: "image", createdAt: "", updatedAt: "", thumbnailAvailable: true },
      { resourceRef: { kind: "asset", id: "a2" }, title: "风景", subtype: "image", createdAt: "", updatedAt: "", thumbnailAvailable: true },
    ],
    nextCursor: null,
    coverage: "complete-category",
    coverageMessage: "",
  });
  api.imageGet.mockResolvedValue([137, 80, 78, 71]);
});
afterEach(() => cleanup());

it("keeps the shared picker large enough to inspect complete images without cropping", () => {
  const css = readFileSync("src/features/create/CreatePage.css", "utf8");
  const rule = (selector: string) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .find(match => match[1].trim() === selector)?.[2] ?? "";
  const panelWidth = Number(/width:\s*min\((\d+)px/.exec(rule(".image-asset-picker"))?.[1]);
  const imageHeight = Number(/height:\s*(\d+)px/.exec(rule(".image-asset-tile .asset-image-thumb"))?.[1]);
  expect(panelWidth).toBeGreaterThanOrEqual(1000);
  expect(imageHeight).toBeGreaterThanOrEqual(200);
  expect(rule(".image-asset-tile .asset-image-thumb")).toMatch(/width:\s*100%/);
  expect(rule(".image-asset-tile .asset-image-thumb img")).toMatch(/object-fit:\s*contain/);
  expect(rule(".image-asset-picker-grid")).toMatch(/grid-auto-rows:\s*max-content/);
  expect(rule(".image-asset-tile-name")).not.toMatch(/max-width:\s*112px/);
});

it("audit: consumes the official next cursor instead of silently hiding later images", async () => {
  const cursor = { projectId: "p1", category: "images", keyword: null, position: { id: "a100", createdAt: "2026-01-01" } };
  const item = (id: string) => ({ resourceRef: { kind: "asset", id }, title: id, subtype: "image", createdAt: "", updatedAt: "", thumbnailAvailable: false });
  api.list.mockImplementation(async (_p, q) => q.cursor ? { items: [item("a1"), item("a101")], nextCursor: null } : { items: [item("a1")], nextCursor: cursor });
  const onSelect = vi.fn();
  render(<ImageAssetPicker projectId="p1" open onSelect={onSelect} onClose={vi.fn()} />);
  const more = await screen.findByRole("button", { name: "加载更多图片" });
  fireEvent.click(more);
  fireEvent.click(more);
  fireEvent.click(await screen.findByRole("option", { name: /a101/ }));
  expect(onSelect).toHaveBeenCalledWith("a101");
  expect(api.list).toHaveBeenLastCalledWith("p1", expect.objectContaining({ cursor }));
  expect(screen.getAllByRole("option").map(option => option.querySelector(".image-asset-tile-name")?.textContent)).toEqual(["a1", "a101"]);
  expect(api.list).toHaveBeenCalledTimes(2);
});

it("keeps current local choices usable while Library is loading", async () => {
  api.list.mockReturnValue(new Promise(() => {}));
  const onSelect = vi.fn();
  render(<ImageAssetPicker projectId="local-project" open onSelect={onSelect} onClose={vi.fn()}
    localChoices={[{ id: "current-local", name: "当前图片", mediaKind: "image" }]} />);
  fireEvent.click(screen.getByRole("option", { name: /当前图片/ }));
  expect(onSelect).toHaveBeenCalledWith("current-local");
});

it("ignores a late continuation after the search scope changes", async () => {
  let finish!: (value: { items: unknown[]; nextCursor: null }) => void;
  const item = (id: string) => ({ resourceRef: { kind: "asset", id }, title: id, subtype: "image", createdAt: "", updatedAt: "", thumbnailAvailable: false });
  api.list.mockImplementation(async (_p, q) => {
    if (q.cursor) return new Promise(yes => { finish = yes; });
    return { items: [item(q.keyword ? "current-search" : "old-search")], nextCursor: q.keyword ? null : { id: "old-cursor" } };
  });
  render(<ImageAssetPicker projectId="p" open onSelect={vi.fn()} onClose={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: "加载更多图片" }));
  await waitFor(() => expect(finish).toBeTypeOf("function"));
  fireEvent.change(screen.getByLabelText("搜索图片"), { target: { value: "current" } });
  await screen.findByRole("option", { name: /current-search/ });
  await act(async () => finish({ items: [item("stale-continuation")], nextCursor: null }));
  expect(screen.queryByRole("option", { name: /stale-continuation/ })).toBeNull();
  expect(screen.queryByRole("option", { name: /old-search/ })).toBeNull();
});

it("audit: changing project cannot select an old project's image during loading", async () => {
  const onSelect = vi.fn(), onClose = vi.fn();
  const view = render(<ImageAssetPicker projectId="p1" open onSelect={onSelect} onClose={onClose} />);
  await screen.findByRole("option", { name: /角色甲/ });
  let finish!: (value: { items: []; nextCursor: null }) => void;
  api.list.mockImplementation(() => new Promise(yes => { finish = yes; }));
  view.rerender(<ImageAssetPicker projectId="p2" open onSelect={onSelect} onClose={onClose} />);
  const stale = screen.queryByRole("option", { name: /角色甲/ });
  if (stale) fireEvent.click(stale);
  expect(onSelect).not.toHaveBeenCalled();
  await waitFor(() => expect(api.list).toHaveBeenLastCalledWith("p2", expect.anything()));
  await act(async () => finish({ items: [], nextCursor: null }));
});

it("shows thumbnail tiles with search and entity tabs, and selects an asset", async () => {
  const onSelect = vi.fn();
  const onClose = vi.fn();
  render(<ImageAssetPicker projectId="p1" open title="选择图片素材" onSelect={onSelect} onClose={onClose}
    localChoices={[{ id: "local1", name: "本地图", mediaKind: "image" }]} />);
  const dialog = await screen.findByRole("dialog", { name: "选择图片素材" });
  await waitFor(() => expect(within(dialog).getByRole("option", { name: /角色甲/ })).toBeTruthy());
  expect(within(dialog).getByRole("tab", { name: "角色" })).toBeTruthy();
  api.list.mockResolvedValue({ items: [{ resourceRef: { kind: "asset", id: "a2" }, title: "风景", subtype: "image", createdAt: "", updatedAt: "", thumbnailAvailable: true }], nextCursor: null });
  fireEvent.change(within(dialog).getByLabelText("搜索图片"), { target: { value: "风景" } });
  expect(within(dialog).queryByRole("option", { name: /角色甲/ })).toBeNull();
  await waitFor(() => expect(api.list).toHaveBeenLastCalledWith("p1", expect.objectContaining({ keyword: "风景" })));
  fireEvent.click(await within(dialog).findByRole("option", { name: /风景/ }));
  expect(onSelect).toHaveBeenCalledWith("a2");
  expect(onClose).toHaveBeenCalled();
});
