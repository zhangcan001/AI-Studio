// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ImageAssetPicker } from "./ImageAssetPicker";

const api = vi.hoisted(() => ({
  list: vi.fn(),
  tagsList: vi.fn(),
  imageGet: vi.fn(),
}));
vi.mock("../../product/client", () => ({ productClient: { library: api } }));

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

it("shows thumbnail tiles with search and entity tabs, and selects an asset", async () => {
  const onSelect = vi.fn();
  const onClose = vi.fn();
  render(<ImageAssetPicker projectId="p1" open title="选择图片素材" onSelect={onSelect} onClose={onClose}
    localChoices={[{ id: "local1", name: "本地图", mediaKind: "image" }]} />);
  const dialog = await screen.findByRole("dialog", { name: "选择图片素材" });
  await waitFor(() => expect(within(dialog).getByRole("option", { name: /角色甲/ })).toBeTruthy());
  expect(within(dialog).getByRole("tab", { name: "角色" })).toBeTruthy();
  fireEvent.change(within(dialog).getByLabelText("搜索图片"), { target: { value: "风景" } });
  await waitFor(() => expect(api.list).toHaveBeenCalled());
  fireEvent.click(within(dialog).getByRole("option", { name: /角色甲/ }));
  expect(onSelect).toHaveBeenCalledWith("a1");
  expect(onClose).toHaveBeenCalled();
});
