import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AssetView } from "../../types/asset";
import { AssetWorkspace, tabs } from "./AssetWorkspace";

const asset: AssetView = {
  id: "asset-1",
  assetType: "image",
  category: "source_image",
  name: "主角参考图",
  originalName: "character.png",
  mimeType: "image/png",
  fileSize: 10,
  createdAt: "2026-01-01T00:00:00Z",
  isFavorite: false,
  tags: [],
};

describe("AssetWorkspace", () => {
  it("exposes the three desktop tabs and keeps 素材 as the default view", () => {
    const html = renderToStaticMarkup(
      <AssetWorkspace
        projectId="project-1"
        onUseInStudio={(_asset: AssetView) => undefined}
        onOpenVideoBatch={(_assets: AssetView[]) => undefined}
        onOpenTask={(_taskId: string) => undefined}
      />,
    );

    expect(tabs.map((tab) => tab.label)).toEqual(["素材", "档案", "参考集"]);
    expect(html).toContain('aria-label="资产工作区"');
    expect(html).toContain('aria-selected="true"');
    expect(html).toContain("<strong>素材</strong>");
    expect(html).toContain("档案");
    expect(html).toContain("参考集");
    expect(html).toContain("资产库");
    expect(asset.id).toBe("asset-1");
  });
});
