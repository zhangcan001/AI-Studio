// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AssetView } from "../../types/asset";
import { AssetLibrary } from "./AssetLibrary";
import { AssetGrid } from "./AssetGrid";

const mocks = vi.hoisted(() => ({
  assetLibraryPage: vi.fn(),
  getAsset: vi.fn(),
  listAssetTags: vi.fn(),
  readAssetImage: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, ...mocks };
});

const asset: AssetView = {
  id: "asset-1",
  assetType: "image",
  category: "source_image",
  name: "主角参考图",
  originalName: "character.png",
  mimeType: "image/png",
  fileSize: 1024,
  createdAt: "2026-09-01T00:00:00Z",
  updatedAt: "2026-09-02T00:00:00Z",
  currentVersionNumber: 3,
  sourceTaskId: "task-1",
  isFavorite: false,
  tags: [{ id: "tag-character", name: "人物" }],
};

const page = { items: [asset], nextCursor: undefined };

describe("Asset Library MVP", () => {
  beforeEach(() => {
    mocks.assetLibraryPage.mockResolvedValue(page);
    mocks.getAsset.mockResolvedValue(asset);
    mocks.listAssetTags.mockResolvedValue([{ id: "tag-character", name: "人物" }]);
    mocks.readAssetImage.mockRejectedValue(new Error("preview unavailable"));
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("renders list metadata, project scope, and explicit empty/loading states", () => {
    const html = renderToStaticMarkup(
      <>
        <AssetGrid projectId="project-1" assets={[asset]} onSelect={vi.fn()} />
        <AssetGrid projectId="project-1" assets={[]} onSelect={vi.fn()} loading />
      </>,
    );

    expect(html).toContain("主角参考图");
    expect(html).toContain("源图片");
    expect(html).toContain("人物");
    expect(html).toContain("当前版本：v3");
    expect(html).toContain("更新时间");
    expect(html).toContain("正在加载素材");
  });

  it("keeps the project scope explicit and applies typed type/tag filters", async () => {
    const user = userEvent.setup();
    render(
      <AssetLibrary
        projectId="project-1"
        onUseInStudio={vi.fn()}
        onOpenVideoBatch={vi.fn()}
        onOpenTask={vi.fn()}
      />,
    );

    await waitFor(() => expect(mocks.assetLibraryPage).toHaveBeenCalledTimes(1));
    expect(screen.getByLabelText("项目筛选：project-1")).toBeTruthy();
    expect(mocks.assetLibraryPage.mock.calls[0][0]).toMatchObject({ projectId: "project-1", mediaType: "ALL", tagId: undefined });

    await user.selectOptions(screen.getByLabelText("类型"), "VIDEO");
    await waitFor(() => expect(mocks.assetLibraryPage).toHaveBeenCalledTimes(2));
    expect(mocks.assetLibraryPage.mock.calls[1][0]).toMatchObject({ projectId: "project-1", mediaType: "VIDEO" });

    await user.selectOptions(screen.getByLabelText("标签"), "tag-character");
    await waitFor(() => expect(mocks.assetLibraryPage).toHaveBeenCalledTimes(3));
    expect(mocks.assetLibraryPage.mock.calls[2][0]).toMatchObject({ projectId: "project-1", mediaType: "VIDEO", tagId: "tag-character" });
  });

  it("keeps the native import action and the project-empty copy", () => {
    const html = renderToStaticMarkup(
      <AssetLibrary
        projectId="project-1"
        onUseInStudio={vi.fn()}
        onOpenVideoBatch={vi.fn()}
        onOpenTask={vi.fn()}
      />,
    );

    expect(html).toContain("导入本地素材");
    expect(html).toContain("当前项目还没有素材");
    expect(html).not.toContain('type="file"');
  });
});
