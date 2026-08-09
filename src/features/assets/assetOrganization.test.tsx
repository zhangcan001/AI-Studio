import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AssetView } from "../../types/asset";
import { AssetCard } from "./AssetCard";
import { replaceAssetOrganization } from "./assetOrganization";

const asset = (id: string, favorite = false): AssetView => ({ id, assetType: "image", category: "source_image", name: id, originalName: `${id}.png`, mimeType: "image/png", fileSize: 1, createdAt: "2026-01-01T00:00:00Z", isFavorite: favorite, tags: favorite ? [{ id: "tag_people", name: "人物" }] : [] });

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
});
