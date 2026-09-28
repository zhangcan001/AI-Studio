// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AssetView } from "../../types/asset";
import { AssetCard } from "./AssetCard";
import { replaceAssetOrganization } from "./assetOrganization";

const mocks = vi.hoisted(() => ({ readAssetImage: vi.fn(), readAssetThumbnail: vi.fn() }));
vi.mock("../../services/tauriClient", async () => ({
  ...await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient"),
  ...mocks,
}));

const asset = (id: string, favorite = false): AssetView => ({ id, assetType: "image", category: "source_image", name: id, originalName: `${id}.png`, mimeType: "image/png", fileSize: 1, createdAt: "2026-01-01T00:00:00Z", isFavorite: favorite, tags: favorite ? [{ id: "tag_people", name: "人物" }] : [] });

afterEach(cleanup);

describe("资产组织界面", () => {
  it("更新收藏和标签时保留对比栏顺序与其他素材", () => {
    const current = [asset("ast_a"), asset("ast_b")];
    const updated = replaceAssetOrganization(current, asset("ast_a", true));
    expect(updated.map((item) => item.id)).toEqual(["ast_a", "ast_b"]);
    expect(updated[0].isFavorite).toBe(true);
    expect(updated[1]).toBe(current[1]);
  });

  it("资产卡同时提供中文收藏标签和可见标签芯片", () => {
    const html = renderToStaticMarkup(<AssetCard projectId="project-1" asset={asset("ast_a", true)} onSelect={() => undefined} onFavorite={() => undefined} />);
    expect(html).toContain("取消收藏素材");
    expect(html).toContain("已收藏");
    expect(html).toContain("人物");
  });

  it("keeps an asset record visible and reports a missing managed file", async () => {
    mocks.readAssetImage.mockRejectedValueOnce(new Error("ASSET_READ_FAILED: ENOENT no such file or directory"));
    render(<AssetCard projectId="project-1" asset={asset("ast_missing_file")} onSelect={() => undefined} />);
    await waitFor(() => expect(screen.getByText("ASSET_FILE_MISSING")).toBeTruthy());
    expect(screen.getByText("ast_missing_file")).toBeTruthy();
  });
});
