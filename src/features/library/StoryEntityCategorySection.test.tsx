// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { StoryEntityCategorySection } from "./StoryEntityCategorySection";
import type { AssetView } from "../../types/asset";
import type { StoryEntityKind } from "../../types/storyEntity";

const api = vi.hoisted(() => ({ list: vi.fn(), create: vi.fn(), update: vi.fn(), delete: vi.fn(), imageGet: vi.fn(), importImages: vi.fn() }));
vi.mock("../../product/client", () => ({
  productClient: {
    storyEntities: { list: api.list, create: api.create, update: api.update, delete: api.delete, importImages: api.importImages },
    library: { imageGet: api.imageGet },
  },
}));
vi.mock("../../components/ImageAssetPicker", () => ({ ImageAssetPicker: () => null }));

afterEach(() => { cleanup(); vi.clearAllMocks(); });

it.each([
  { kind: "character" as StoryEntityKind, label: "角色", filename: "苏蔓.png" },
  { kind: "scene" as StoryEntityKind, label: "场景", filename: "夜晚街道.webp" },
  { kind: "prop" as StoryEntityKind, label: "道具", filename: "古剑.jpg" },
  { kind: "composition" as StoryEntityKind, label: "构图", filename: "剧情01.png" },
])("bulk imports $label images as separate entities named from their filenames", async ({ kind, label, filename }) => {
  api.list.mockResolvedValue([]);
  api.importImages.mockResolvedValue({
    imported: [{ id: "asset-1", name: filename, originalName: filename } as AssetView],
    failed: [],
    cancelled: false,
  });
  api.create.mockResolvedValue({});

  render(<StoryEntityCategorySection projectId="project-a" kind={kind} />);
  fireEvent.click(await screen.findByRole("button", { name: `批量导入${label}图片` }));

  await waitFor(() => expect(api.create).toHaveBeenCalledWith({
    projectId: "project-a",
    kind,
    name: filename.slice(0, filename.lastIndexOf(".")),
    description: "",
    coverAssetId: "asset-1",
    imageAssetIds: ["asset-1"],
  }));
  expect(await screen.findByText(new RegExp(`批量导入完成：成功创建 1 个${label}`))).toBeTruthy();
  expect(api.list).toHaveBeenLastCalledWith("project-a", kind);
});

it("does not create entities when the image picker is cancelled", async () => {
  api.list.mockResolvedValue([]);
  api.importImages.mockResolvedValue({ imported: [], failed: [], cancelled: true });
  render(<StoryEntityCategorySection projectId="project-a" kind="scene" />);
  await waitFor(() => expect(screen.getByRole("button", { name: "批量导入场景图片" }).hasAttribute("disabled")).toBe(false));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "批量导入场景图片" })); });
  expect(api.importImages).toHaveBeenCalledWith("project-a");
  expect(api.create).not.toHaveBeenCalled();
  expect(screen.queryByText(/批量导入完成/)).toBeNull();
});

it("reports partial failures without discarding successfully imported images", async () => {
  api.list.mockResolvedValue([]);
  api.importImages.mockResolvedValue({
    imported: [
      { id: "asset-good", name: "古剑.jpg", originalName: "古剑.jpg" },
      { id: "asset-duplicate", name: "旧剑.png", originalName: "旧剑.png" },
    ],
    failed: [{ displayName: "损坏.png", error: "格式不合法" }], cancelled: false,
  });
  api.create.mockResolvedValueOnce({}).mockRejectedValueOnce(new Error("名称已存在"));
  render(<StoryEntityCategorySection projectId="project-a" kind="prop" />);
  await waitFor(() => expect(screen.getByRole("button", { name: "批量导入道具图片" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "批量导入道具图片" }));
  expect(await screen.findByText(/成功创建 1 个道具.*失败 2 项/)).toBeTruthy();
  expect(screen.getByText(/已导入的图片仍保留在媒体库/)).toBeTruthy();
  expect(api.create).toHaveBeenCalledTimes(2);
  expect(api.delete).not.toHaveBeenCalled();
});

it("ignores a late list response after the keyed project/category view changes", async () => {
  let resolveOld!: (value: unknown[]) => void;
  api.list.mockImplementation((project: string) => project === "project-a"
    ? new Promise(resolve => { resolveOld = resolve; })
    : Promise.resolve([{ id: "scene-b", projectId: "project-b", kind: "scene", name: "新场景", description: "", coverAssetId: null, imageAssetIds: [] }]));
  const view = render(<StoryEntityCategorySection key="a:character" projectId="project-a" kind="character" />);
  view.rerender(<StoryEntityCategorySection key="b:scene" projectId="project-b" kind="scene" />);
  expect(await screen.findByText("新场景")).toBeTruthy();
  await act(async () => { resolveOld([{ id: "old-character", name: "迟到的旧角色", description: "", coverAssetId: null, imageAssetIds: [] }]); });
  expect(screen.queryByText("迟到的旧角色")).toBeNull();
  expect(screen.getByRole("region", { name: "场景管理" })).toBeTruthy();
});
